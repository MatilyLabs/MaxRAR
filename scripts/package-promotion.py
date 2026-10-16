#!/usr/bin/env python3
"""Package an already signed payload, preserving the exact signed UTF-8 bytes."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('payload', type=Path)
parser.add_argument('signature', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
payload = args.payload.read_bytes().decode('utf-8')
json.loads(payload)
signature = args.signature.read_bytes().decode('utf-8')
envelope = json.dumps({'payload': payload, 'signature': signature}, ensure_ascii=False).encode('utf-8')
if len(envelope) > 65536:
    parser.error('Signed feed exceeds 64 KiB')
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_bytes(envelope)
