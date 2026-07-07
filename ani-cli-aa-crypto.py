#!/usr/bin/env python3
import base64
import hashlib
import json
import math
import sys
import time

try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    sys.stderr.write('Python package "cryptography" is required for AllAnime episode crypto.\n')
    sys.exit(1)


BUILD_ID = "9"
EPOCH = 4128
KEY_A_HEX = "b1a9a4d051988f1b1b12dbb747439d9bd64b09ea17835600a7eaa4de87c1ad87"
KEY_B_B64 = "k7DLdv5SGiuEyGUtcncl5wQOR7r4aenLfDV3AOBKlAU="
VERSION = 1
TS_BUCKET_MS = 300_000


def key() -> bytes:
    key_a = bytes.fromhex(KEY_A_HEX)
    key_b = base64.b64decode(KEY_B_B64)
    return bytes(a ^ b for a, b in zip(key_a, key_b))


def iv(query_hash: str, ts: int) -> bytes:
    seed = f"{EPOCH}:{BUILD_ID}:{query_hash}:{ts}".encode()
    return hashlib.sha256(seed).digest()[:12]


def sign(query_hash: str) -> str:
    ts = math.floor((time.time() * 1000) / TS_BUCKET_MS) * TS_BUCKET_MS
    payload = {
        "v": VERSION,
        "ts": ts,
        "epoch": EPOCH,
        "buildId": BUILD_ID,
        "qh": query_hash,
    }
    nonce = iv(query_hash, ts)
    plaintext = json.dumps(payload, separators=(",", ":")).encode()
    encrypted = AESGCM(key()).encrypt(nonce, plaintext, None)
    return base64.b64encode(bytes([VERSION]) + nonce + encrypted).decode()


def decrypt_tobeparsed(value: str) -> str:
    envelope = base64.b64decode(value)
    if not envelope or envelope[0] != VERSION:
        raise ValueError("Unsupported AllAnime encrypted response version")
    nonce = envelope[1:13]
    encrypted = envelope[13:]
    return AESGCM(key()).decrypt(nonce, encrypted, None).decode()


def decrypt_response() -> str:
    response = json.load(sys.stdin)
    return decrypt_tobeparsed(response["data"]["tobeparsed"])


def main() -> int:
    if len(sys.argv) < 2:
        sys.stderr.write("usage: ani-cli-aa-crypto.py sign QUERY_HASH | decrypt\n")
        return 2
    if sys.argv[1] == "sign" and len(sys.argv) == 3:
        print(sign(sys.argv[2]), end="")
        return 0
    if sys.argv[1] == "decrypt":
        print(decrypt_response(), end="")
        return 0
    sys.stderr.write("usage: ani-cli-aa-crypto.py sign QUERY_HASH | decrypt\n")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
