use {
    super::{Error, INGEST_TO_DECODER_CAPACITY, Ingester},
    crate::types::{
        Signature,
        channel::StreamUpdate,
        slot::Slot,
        wire::{
            SubscribeUpdate,
            SubscribeUpdateAccount,
            SubscribeUpdateAccountInfo,
            SubscribeUpdatePing,
            SubscribeUpdateSlot,
            SubscribeUpdateTransaction,
            SubscribeUpdateTransactionInfo,
            UpdateOneof,
        },
    },
    futures::stream,
    std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    tokio::sync::mpsc::channel,
    // Update variants the ingester ignores by falling through its match. Pulled
    // from the proto crate directly rather than the curated `wire` surface.
    yellowstone_grpc_proto::{
        geyser::{
            SubscribeUpdateBlock,
            SubscribeUpdateBlockMeta,
            SubscribeUpdateEntry,
            SubscribeUpdatePong,
            SubscribeUpdateTransactionStatus,
        },
        tonic::Status,
    },
};

fn signature(n: u8) -> Signature {
    Signature::from([n; 64])
}

fn signature_bytes(n: u8) -> Vec<u8> {
    signature(n).as_ref().to_vec()
}

fn tx_update(slot: u64, sig: u8) -> Result<SubscribeUpdate, Status> {
    Ok(SubscribeUpdate {
        update_oneof: Some(UpdateOneof::Transaction(SubscribeUpdateTransaction {
            slot,
            transaction: Some(SubscribeUpdateTransactionInfo {
                signature: signature_bytes(sig),
                ..Default::default()
            }),
        })),
        ..Default::default()
    })
}

fn account_update(slot: u64, sig: u8) -> Result<SubscribeUpdate, Status> {
    Ok(SubscribeUpdate {
        update_oneof: Some(UpdateOneof::Account(SubscribeUpdateAccount {
            slot,
            account: Some(SubscribeUpdateAccountInfo {
                txn_signature: Some(signature_bytes(sig)),
                ..Default::default()
            }),
            ..Default::default()
        })),
        ..Default::default()
    })
}

fn slot_update(slot: u64) -> Result<SubscribeUpdate, Status> {
    Ok(SubscribeUpdate {
        update_oneof: Some(UpdateOneof::Slot(SubscribeUpdateSlot {
            slot,
            ..Default::default()
        })),
        ..Default::default()
    })
}

fn update_of(update: UpdateOneof) -> Result<SubscribeUpdate, Status> {
    Ok(SubscribeUpdate {
        update_oneof: Some(update),
        ..Default::default()
    })
}

fn ingester(
    stream: impl stream::Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send,
) -> (
    Ingester<impl stream::Stream<Item = Result<SubscribeUpdate, Status>> + Unpin + Send>,
    tokio::sync::mpsc::Receiver<StreamUpdate>,
    Arc<AtomicU64>,
) {
    let (tx, rx) = channel(INGEST_TO_DECODER_CAPACITY);
    let latest_chain_slot = Arc::new(AtomicU64::new(0));
    (
        Ingester::new(stream, tx, latest_chain_slot.clone()),
        rx,
        latest_chain_slot,
    )
}

#[tokio::test]
async fn transaction_update_with_valid_signature_is_forwarded() {
    let signature = signature(1);
    let (mut ingester, mut rx, _) = ingester(stream::iter([tx_update(42, 1)]));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
    let update = rx.recv().await.unwrap();
    assert!(
        matches!(update, StreamUpdate::Tx { slot: Slot(42), signature: s, .. } if s == signature)
    );
    assert!(rx.is_empty());
}

/// Account updates are not part of the tx-only subscription, so the ingester
/// drops them and forwards nothing.
#[tokio::test]
async fn account_update_is_ignored() {
    let (mut ingester, rx, _) = ingester(stream::iter([account_update(100, 2)]));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
    assert!(rx.is_empty());
}

#[tokio::test]
async fn slot_update_advances_latest_chain_slot() {
    let (mut ingester, _rx, slot) = ingester(stream::iter([slot_update(9_001)]));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
    assert_eq!(slot.load(Ordering::Relaxed), 9_001);
}

#[tokio::test]
async fn unrelated_and_empty_updates_are_ignored() {
    let (mut ingester, mut rx, slot) = ingester(stream::iter([
        Ok(SubscribeUpdate::default()),
        update_of(UpdateOneof::Ping(SubscribeUpdatePing::default())),
        update_of(UpdateOneof::Pong(SubscribeUpdatePong::default())),
        update_of(UpdateOneof::TransactionStatus(
            SubscribeUpdateTransactionStatus::default(),
        )),
        update_of(UpdateOneof::Block(SubscribeUpdateBlock::default())),
        update_of(UpdateOneof::BlockMeta(SubscribeUpdateBlockMeta::default())),
        update_of(UpdateOneof::Entry(SubscribeUpdateEntry::default())),
        tx_update(7, 3),
    ]));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
    let update = rx.recv().await.unwrap();
    assert!(
        matches!(update, StreamUpdate::Tx { slot: Slot(7), signature: s, .. } if s == signature(3))
    );
    assert!(rx.is_empty());
    assert_eq!(slot.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn transaction_without_body_or_malformed_signature_is_skipped() {
    let signature = signature(4);
    let (mut ingester, mut rx, _) = ingester(stream::iter([
        Ok(SubscribeUpdate {
            update_oneof: Some(UpdateOneof::Transaction(SubscribeUpdateTransaction {
                slot: 1,
                transaction: None,
            })),
            ..Default::default()
        }),
        Ok(SubscribeUpdate {
            update_oneof: Some(UpdateOneof::Transaction(SubscribeUpdateTransaction {
                slot: 2,
                transaction: Some(SubscribeUpdateTransactionInfo {
                    signature: vec![1, 2, 3],
                    ..Default::default()
                }),
            })),
            ..Default::default()
        }),
        tx_update(3, 4),
    ]));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
    let update = rx.recv().await.unwrap();
    assert!(
        matches!(update, StreamUpdate::Tx { slot: Slot(3), signature: s, .. } if s == signature)
    );
    assert!(rx.is_empty());
}

#[tokio::test]
async fn terminal_grpc_status_returns_stream_error() {
    let status = Status::invalid_argument("boom");
    let (mut ingester, _rx, _) = ingester(stream::iter([Err(status.clone())]));

    let result = ingester.run().await;
    assert!(
        matches!(result, Err(Error::Stream(s)) if s.code() == status.code() && s.message() == status.message())
    );
}

#[tokio::test]
async fn clean_stream_end_returns_stream_ended() {
    let (mut ingester, _rx, _) =
        ingester(stream::iter(Vec::<Result<SubscribeUpdate, Status>>::new()));

    assert!(matches!(ingester.run().await, Err(Error::StreamEnded)));
}

#[tokio::test]
async fn closed_decoder_receiver_stops_cleanly() {
    let (mut ingester, rx, _) = ingester(stream::iter([tx_update(1, 5)]));
    drop(rx);

    assert!(ingester.run().await.is_ok());
}
