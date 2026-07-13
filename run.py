#!/opt/homebrew/bin/python3

import datetime
import pathlib
import subprocess
import sys

LOG_DIR = pathlib.Path("run-logs")

NETWORKS = [
    # ("sepolia", "eth-sepolia"),
    # ("ink", "ink-mainnet"),
    # ("linea", "linea-mainnet"),
    # ("plasma", "plasma-mainnet"),
    # ("arbitrum-one", "arb-mainnet"),
    ("polygon", "polygon-mainnet"),
    ("avalanche", "avax-mainnet"),
    ("xdai", "gnosis-mainnet"),
    ("bnb", "bnb-mainnet"),
    ("base", "base-mainnet"),
    ("mainnet", "eth-mainnet"),
]


def staging_db(network: str) -> str:
    return f"postgresql://cow-protocol-db-read-replica.cgabamo3x0wl.eu-central-1.rds.amazonaws.com:5432/{network}?user=cow_protocol_admin&password=Ix4Aenaew9vai4iezair8aivuih0ie"


def prod_db(network: str) -> str:
    return f"postgresql://localhost:5432/{network}?user=gp_readonly&password=eb7AiZeekong7uy3ua9aD5oku8idae"


def alchemy_rpc(network: str) -> str:
    return f"https://{network}.g.alchemy.com/v2/IeY0r41K55FoNXURcBx4P"


def settlements(network: str) -> list[str]:
    if network == "mainnet":
        return [
            "--settlement",
            "0x4E608b7Da83f8E9213F554BDAA77C72e125529d0:-12369542",
            "--settlement",
            "0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf:11973117-17502498",
            "--settlement",
            "0x9008D19f58AAbD9eD0D60971565AA8510560ab41:12593265-",
        ]
    elif network == "xdai":
        return [
            "--settlement",
            "0x4E608b7Da83f8E9213F554BDAA77C72e125529d0:-15309608",
            "--settlement",
            "0x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf:14846022-17739439",
            "--settlement",
            "0x9008D19f58AAbD9eD0D60971565AA8510560ab41:16465100-",
        ]

    else:
        return [
            "--settlement",
            "0x9008D19f58AAbD9eD0D60971565AA8510560ab41",
        ]


def run(name: str, command: list[str]) -> int:
    """Run `command`, streaming its combined stdout+stderr to the terminal and
    to run-logs/<name>.log at the same time (like `tee`). Returns the exit
    code."""
    LOG_DIR.mkdir(exist_ok=True)
    # Same UTC stamp format the tool names its reports with (YYYYMMDDThhmmssZ),
    # so a log and its report sort together and history is never overwritten.
    timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    log_path = LOG_DIR / f"{name}-{timestamp}.log"

    print(f"=== {name}: {' '.join(command)}", flush=True)
    with (
        open(log_path, "w") as log,
        subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,  # line-buffered, so output appears as it is produced
        ) as proc,
    ):
        assert proc.stdout is not None
        for line in proc.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            log.write(line)
            log.flush()

    return proc.returncode


results: dict[str, int] = {}

for env in ["prod"]:
    for db, alchemy in NETWORKS:
        db_conn_str = staging_db(db) if env == "staging" else prod_db(db)
        alchemy_conn_str = alchemy_rpc(alchemy)
        settlement = settlements(db)

        command = [
            "cargo",
            "r",
            "-p",
            "settlement-finder",
            "-r",
            "--",
            "verify",
            "--rpc-url",
            alchemy_conn_str,
            "--db",
            db_conn_str,
            "--concurrency",
            "10",
            "--chunk",
            "5000",
            *settlement,
        ]

        results[f"{env}-{db}"] = run(f"{env}-{db}", command)

print("\n=== summary ===")
for name, code in results.items():
    print(f"{name}: exit {code}")
