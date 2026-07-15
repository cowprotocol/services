//! Proof-of-concept: archive an object to S3 using brotli compression (the
//! codec the autopilot already uses for solve requests) and verify it can be
//! retrieved and decompressed correctly.
//!
//! Purpose: de-risk switching S3 auction archival (`crates/s3/src/lib.rs`,
//! currently gzip level 3) to brotli-1, which this codebase already found to
//! beat gzip-3 on both ratio and speed for these JSON payloads (see
//! `crates/autopilot/src/infra/solvers/dto/solve.rs`).
//!
//! AWS credentials and region are loaded from the environment, exactly like
//! `s3::Uploader::new`.
//!
//! Examples (S3 subcommands need AWS creds + `--bucket`/`BUCKET`):
//!   cargo run -p s3-brotli-poc -- local ./auction.json
//!   cargo run -p s3-brotli-poc -- upload ./auction.json
//!   cargo run -p s3-brotli-poc -- download --expect ./auction.json
//!   cargo run -p s3-brotli-poc -- roundtrip --cleanup

use {
    anyhow::{Context, Result, ensure},
    aws_sdk_s3::{
        Client,
        primitives::{ByteStream, SdkBody},
    },
    bytes::Bytes,
    clap::{Parser, Subcommand},
    std::{io::Write, path::PathBuf},
};

/// brotli quality 1, lgwin 22, 4 KiB buffer — identical to the solve-request
/// path in `crates/autopilot/src/infra/solvers/dto/solve.rs`.
const BROTLI_QUALITY: u32 = 1;
const BROTLI_LGWIN: u32 = 22;
const BROTLI_BUFFER: usize = 4096;

const DEFAULT_KEY: &str = "brotli-poc/poc.json";

#[derive(Parser)]
#[command(about = "Upload brotli-compressed objects to S3 and verify retrieval")]
struct Cli {
    /// Target S3 bucket. Not needed for the `local` subcommand.
    #[arg(long, env = "BUCKET", global = true)]
    bucket: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Compress a file with brotli and upload it (Content-Encoding: br).
    Upload {
        /// File to upload. If omitted, a synthetic ~4 MB JSON payload is used.
        file: Option<PathBuf>,
        /// Object key. Defaults to `brotli-poc/poc.json`.
        #[arg(long)]
        key: Option<String>,
    },
    /// Download an object, decompress it with brotli, and optionally verify it.
    Download {
        /// Object key to fetch. Defaults to `brotli-poc/poc.json`.
        key: Option<String>,
        /// Assert the decompressed bytes equal this file's contents.
        #[arg(long)]
        expect: Option<PathBuf>,
        /// Write the decompressed bytes to this path.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Upload, then immediately download and verify a full round-trip
    /// (default).
    Roundtrip {
        /// File to use. If omitted, a synthetic ~4 MB JSON payload is used.
        file: Option<PathBuf>,
        #[arg(long)]
        key: Option<String>,
        /// Delete the object from S3 after a successful round-trip.
        #[arg(long)]
        cleanup: bool,
    },
    /// Offline: brotli-compress then decompress in memory and verify equality.
    /// Proves the codec without any AWS access.
    Local {
        /// File to use. If omitted, a synthetic ~4 MB JSON payload is used.
        file: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let Cli { bucket, command } = Cli::parse();
    let command = command.unwrap_or(Command::Roundtrip {
        file: None,
        key: None,
        cleanup: false,
    });
    let bucket = || {
        bucket
            .clone()
            .context("--bucket (or env BUCKET) is required")
    };

    match command {
        Command::Local { file } => {
            let plaintext = load_or_synthetic(file)?;
            let compressed = brotli_compress(&plaintext)?;
            report_ratio(plaintext.len(), compressed.len());
            let decoded = brotli_decompress(&compressed)?;
            ensure!(decoded == plaintext, "local round-trip mismatch");
            println!("✓ local brotli round-trip OK ({} bytes)", plaintext.len());
        }
        Command::Upload { file, key } => {
            let bucket = bucket()?;
            let key = key.unwrap_or_else(|| DEFAULT_KEY.to_string());
            let plaintext = load_or_synthetic(file)?;
            upload(&client().await, &bucket, &key, &plaintext).await?;
        }
        Command::Download { key, expect, out } => {
            let bucket = bucket()?;
            let key = key.unwrap_or_else(|| DEFAULT_KEY.to_string());
            let (decoded, encoding) = download(&client().await, &bucket, &key).await?;
            ensure!(
                encoding.as_deref() == Some("br"),
                "expected stored Content-Encoding 'br', got {encoding:?}"
            );
            if let Some(path) = out {
                std::fs::write(&path, &decoded)
                    .with_context(|| format!("write {}", path.display()))?;
                println!("wrote {} bytes to {}", decoded.len(), path.display());
            }
            if let Some(path) = expect {
                let want =
                    std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
                ensure!(decoded == want, "decompressed bytes != {}", path.display());
                println!("✓ retrieved object matches {}", path.display());
            }
        }
        Command::Roundtrip { file, key, cleanup } => {
            let bucket = bucket()?;
            let key = key.unwrap_or_else(|| DEFAULT_KEY.to_string());
            let client = client().await;
            let plaintext = load_or_synthetic(file)?;
            upload(&client, &bucket, &key, &plaintext).await?;
            let (decoded, encoding) = download(&client, &bucket, &key).await?;
            ensure!(
                encoding.as_deref() == Some("br"),
                "expected stored Content-Encoding 'br', got {encoding:?}"
            );
            ensure!(
                decoded == plaintext,
                "round-trip mismatch: {} decoded vs {} original bytes",
                decoded.len(),
                plaintext.len()
            );
            println!(
                "✓ S3 brotli round-trip OK: uploaded, retrieved and decompressed identically ({} \
                 bytes)",
                plaintext.len()
            );
            if cleanup {
                client
                    .delete_object()
                    .bucket(&bucket)
                    .key(&key)
                    .send()
                    .await
                    .context("delete_object")?;
                println!("cleaned up s3://{bucket}/{key}");
            }
        }
    }
    Ok(())
}

async fn client() -> Client {
    Client::new(&aws_config::from_env().load().await)
}

async fn upload(client: &Client, bucket: &str, key: &str, plaintext: &[u8]) -> Result<()> {
    let compressed = brotli_compress(plaintext)?;
    report_ratio(plaintext.len(), compressed.len());
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::new(SdkBody::from(Bytes::from(compressed))))
        .content_encoding("br")
        .content_type("application/json")
        .send()
        .await
        .context("put_object")?;
    println!("uploaded s3://{bucket}/{key} (Content-Encoding: br)");
    Ok(())
}

/// Returns the decompressed payload and the object's stored `Content-Encoding`.
async fn download(client: &Client, bucket: &str, key: &str) -> Result<(Vec<u8>, Option<String>)> {
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("get_object")?;
    let encoding = resp.content_encoding().map(str::to_string);
    let raw = resp.body.collect().await.context("read body")?.to_vec();
    println!(
        "downloaded s3://{bucket}/{key}: {} stored bytes, Content-Encoding: {:?}",
        raw.len(),
        encoding.as_deref(),
    );
    // The AWS SDK returns the bytes exactly as stored and does NOT act on
    // Content-Encoding, so we decompress ourselves — the same contract the
    // current gzip path relies on. If some intermediary had transparently
    // decoded the body, `raw` would already be plaintext and this would error
    // instead of silently returning the wrong thing.
    let plaintext = brotli_decompress(&raw)
        .context("brotli-decompressing the downloaded body (was it stored compressed?)")?;
    println!("decompressed to {} bytes", plaintext.len());
    Ok((plaintext, encoding))
}

fn brotli_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = brotli::enc::writer::CompressorWriter::new(
        Vec::new(),
        BROTLI_BUFFER,
        BROTLI_QUALITY,
        BROTLI_LGWIN,
    );
    encoder.write_all(data).context("brotli write")?;
    encoder.flush().context("brotli flush")?;
    Ok(encoder.into_inner())
}

fn brotli_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    brotli::BrotliDecompress(&mut &data[..], &mut out).context("brotli decompress")?;
    Ok(out)
}

fn report_ratio(original: usize, compressed: usize) {
    let pct = if original == 0 {
        0.0
    } else {
        100.0 * compressed as f64 / original as f64
    };
    let ratio = if compressed == 0 {
        0.0
    } else {
        original as f64 / compressed as f64
    };
    println!(
        "brotli-{BROTLI_QUALITY}: {original} -> {compressed} bytes ({pct:.1}% of original, \
         {ratio:.2}x)"
    );
}

fn load_or_synthetic(file: Option<PathBuf>) -> Result<Vec<u8>> {
    match file {
        Some(path) => std::fs::read(&path).with_context(|| format!("read {}", path.display())),
        None => {
            let payload = synthetic_payload();
            println!("using synthetic payload: {} bytes", payload.len());
            Ok(payload)
        }
    }
}

/// Builds a ~4 MB JSON payload resembling an auction (many similar order
/// entries → realistic compression ratio), without depending on the domain
/// types.
fn synthetic_payload() -> Vec<u8> {
    let mut s = String::from("{\"id\":1,\"orders\":[");
    let mut i = 0u64;
    while s.len() < 4 * 1024 * 1024 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"uid\":\"0x{i:064x}\",\"sellToken\":\"0x{:040x}\",\"buyToken\":\"0x{:040x}\",\"\
             sellAmount\":\"{}\",\"buyAmount\":\"{}\",\"validTo\":{},\"kind\":\"sell\",\"\
             partiallyFillable\":false}}",
            i % 97,
            i % 89,
            1_000_000_000_000_000_000u64 + i,
            2_000_000u64 + i,
            1_700_000_000u64 + i,
        ));
        i += 1;
    }
    s.push_str("]}");
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brotli_round_trips_and_shrinks() {
        let plaintext = synthetic_payload();
        let compressed = brotli_compress(&plaintext).unwrap();
        assert!(
            compressed.len() < plaintext.len(),
            "compressed {} should be < original {}",
            compressed.len(),
            plaintext.len()
        );
        let decoded = brotli_decompress(&compressed).unwrap();
        assert_eq!(decoded, plaintext);
    }
}
