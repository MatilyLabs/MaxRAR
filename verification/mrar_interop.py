#!/usr/bin/env python3
"""Independent MRAR/0 and grouped MRAR/1 smoke reader/writer; libzstd is QA-only.

This intentionally does not import MaxRAR code. It checks generated disposable
fixtures, not untrusted archives, and is not an alternative production extractor.
"""
import argparse
import ctypes
import ctypes.util
import hashlib
import json
import pathlib
import random
import struct
import subprocess
import tempfile

MAGIC = b'MRAR\r\n\x1a\n'
CHUNK = 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f'duplicate field {key}')
        result[key] = value
    return result


def zstd_library():
    name = ctypes.util.find_library('zstd')
    if not name:
        raise RuntimeError('Independent QA requires libzstd; the MaxRAR app does not.')
    lib = ctypes.CDLL(name)
    lib.ZSTD_decompress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
    lib.ZSTD_decompress.restype = ctypes.c_size_t
    lib.ZSTD_compress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int]
    lib.ZSTD_compress.restype = ctypes.c_size_t
    lib.ZSTD_compressBound.argtypes = [ctypes.c_size_t]
    lib.ZSTD_compressBound.restype = ctypes.c_size_t
    lib.ZSTD_isError.argtypes = [ctypes.c_size_t]
    lib.ZSTD_isError.restype = ctypes.c_uint
    lib.ZSTD_findFrameCompressedSize.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    lib.ZSTD_findFrameCompressedSize.restype = ctypes.c_size_t
    return lib


def read_fixture(path, lib):
    raw = path.read_bytes()
    magic, version, flags, length = struct.unpack('<8sHHI', raw[:16])
    assert magic == MAGIC and (version, flags) in {(0, 0), (1, 1)}
    assert 0 < length <= 8 * CHUNK and 16 + length <= len(raw) - 32
    assert hashlib.sha256(raw[:-32]).digest() == raw[-32:]
    def decompress(payload, size):
        assert payload[:4] == b'\x28\xb5\x2f\xfd'
        assert lib.ZSTD_findFrameCompressedSize(payload, len(payload)) == len(payload)
        buffer = ctypes.create_string_buffer(size)
        actual = lib.ZSTD_decompress(buffer, size, payload, len(payload))
        assert not lib.ZSTD_isError(actual) and actual == size
        return buffer.raw[:actual]

    manifest = raw[16:16 + length]
    if version == 1:
        decoded_length, = struct.unpack('<I', manifest[:4])
        assert 0 < decoded_length <= 8 * CHUNK
        manifest = decompress(manifest[4:], decoded_length)
    index = json.loads(manifest, object_pairs_hook=unique_object)
    assert set(index) <= {'version', 'entries', 'groups'} and index['version'] == version
    if version == 0:
        assert set(index) == {'version', 'entries'}
    assert len(index['entries']) <= 100_000
    files, codecs, ranges = {}, set(), []

    def decode_block(block, limit):
        assert set(block) == {'codec', 'offset', 'encoded_size', 'decoded_size', 'sha256'}
        enc, dec = block['encoded_size'], block['decoded_size']
        assert 0 < enc <= dec <= limit
        start = 16 + length + block['offset']
        assert start + enc <= len(raw) - 32
        payload = raw[start:start + enc]
        ranges.append((block['offset'], block['offset'] + enc))
        codecs.add(block['codec'])
        if block['codec'] == 'store':
            assert enc == dec
            output = payload
        else:
            assert block['codec'] == 'zstd'
            output = decompress(payload, dec)
        assert sha(output) == block['sha256']
        return output

    groups = [decode_block(block, 8 * CHUNK) for block in index.get('groups', [])]
    spans = [[] for _ in groups]
    for entry in index['entries']:
        assert set(entry) in ({'path', 'kind', 'size', 'sha256', 'blocks'},
                              {'path', 'kind', 'size', 'sha256', 'blocks', 'group'})
        if entry['kind'] == 'directory':
            assert entry['size'] == 0 and entry['sha256'] == '' and entry['blocks'] == []
            continue
        assert entry['kind'] == 'file' and entry['path'] not in files
        assert 0 <= entry['size'] <= 2 * 1024**3
        decoded = bytearray()
        if entry.get('group') is not None:
            assert version == 1 and not entry['blocks'] and 0 < entry['size'] <= 256 * 1024
            span = entry['group']
            assert set(span) == {'id', 'offset'} and 0 <= span['id'] < len(groups)
            group = groups[span['id']]
            start, end = span['offset'], span['offset'] + entry['size']
            assert 0 <= start < end <= len(group)
            spans[span['id']].append((start, end))
            decoded.extend(group[start:end])
        else:
            for block in entry['blocks']:
                decoded.extend(decode_block(block, CHUNK))
        assert len(decoded) == entry['size'] and sha(decoded) == entry['sha256']
        files[entry['path']] = bytes(decoded)
    for group, members in zip(groups, spans):
        end = 0
        for start, next_end in sorted(members):
            assert start == end
            end = next_end
        assert end == len(group)
    offset = 0
    for start, end in sorted(ranges):
        assert start == offset
        offset = end
    assert 16 + length + offset == len(raw) - 32
    return files, codecs


def write_fixture(path, files, lib):
    entries, data = [], bytearray()
    for name, contents in files.items():
        blocks = []
        for start in range(0, len(contents), CHUNK):
            chunk = contents[start:start + CHUNK]
            capacity = lib.ZSTD_compressBound(len(chunk))
            buffer = ctypes.create_string_buffer(capacity)
            size = lib.ZSTD_compress(buffer, capacity, chunk, len(chunk), 3)
            assert not lib.ZSTD_isError(size)
            codec, encoded = ('zstd', buffer.raw[:size]) if size < len(chunk) else ('store', chunk)
            blocks.append({'codec': codec, 'offset': len(data), 'encoded_size': len(encoded),
                           'decoded_size': len(chunk), 'sha256': sha(chunk)})
            data.extend(encoded)
        entries.append({'path': name, 'kind': 'file', 'size': len(contents),
                        'sha256': sha(contents), 'blocks': blocks})
    index = json.dumps({'version': 0, 'entries': entries}, separators=(',', ':')).encode()
    raw = struct.pack('<8sHHI', MAGIC, 0, 0, len(index)) + index + data
    path.write_bytes(raw + hashlib.sha256(raw).digest())


def write_grouped_fixture(path, files, lib):
    """Independently create /1 with a stored shared group and compressed index."""
    entries, data, group = [], bytearray(), bytearray()
    # These QA fixtures are bounded; the production writer streams bounded groups.
    large = {}
    for name, contents in files.items():
        if 0 < len(contents) <= 256 * 1024:
            entries.append({'path': name, 'kind': 'file', 'size': len(contents), 'sha256': sha(contents),
                            'blocks': [], 'group': {'id': 0, 'offset': len(group)}})
            group.extend(contents)
        else:
            large[name] = contents
    assert 0 < len(group) <= 8 * CHUNK
    groups = [{'codec': 'store', 'offset': 0, 'encoded_size': len(group),
               'decoded_size': len(group), 'sha256': sha(group)}]
    data.extend(group)
    for name, contents in large.items():
        blocks = []
        for start in range(0, len(contents), CHUNK):
            chunk = contents[start:start + CHUNK]
            blocks.append({'codec': 'store', 'offset': len(data), 'encoded_size': len(chunk),
                           'decoded_size': len(chunk), 'sha256': sha(chunk)})
            data.extend(chunk)
        entries.append({'path': name, 'kind': 'file', 'size': len(contents), 'sha256': sha(contents), 'blocks': blocks})
    index = json.dumps({'version': 1, 'entries': entries, 'groups': groups}, separators=(',', ':')).encode()
    buffer = ctypes.create_string_buffer(lib.ZSTD_compressBound(len(index)))
    size = lib.ZSTD_compress(buffer, len(buffer), index, len(index), 3)
    assert not lib.ZSTD_isError(size)
    encoded = struct.pack('<I', len(index)) + buffer.raw[:size]
    raw = struct.pack('<8sHHI', MAGIC, 1, 1, len(encoded)) + encoded + data
    path.write_bytes(raw + hashlib.sha256(raw).digest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=pathlib.Path, default=pathlib.Path('target/debug/maxrar-cli'))
    parser.add_argument('--driver', type=pathlib.Path, help='Use the minimal archive-core driver instead of CLI')
    parser.add_argument('--output', type=pathlib.Path, default=pathlib.Path('verification/mrar-interop.json'))
    args = parser.parse_args()
    cli, lib = (args.driver or args.cli).resolve(strict=True), zstd_library()

    def run(*parts):
        if args.driver:
            if parts[0] == 'create':
                parts = ('create', parts[2], parts[3])
            elif parts[0] == 'extract':
                parts = ('extract', parts[1], parts[3])
            command = [str(cli), *map(str, parts)]
        else:
            command = [str(cli), *map(str, parts), '--json']
        result = subprocess.run(command, capture_output=True, timeout=120)
        assert result.returncode == 0, result.stdout.decode(errors='replace') + result.stderr.decode(errors='replace')
        return json.loads(result.stdout)

    with tempfile.TemporaryDirectory(prefix='maxrar-independent-mrar-') as scratch:
        root = pathlib.Path(scratch)
        source = root / 'source'
        source.mkdir()
        files = {'source/text.txt': b'Independent MRAR compatibility fixture\n' * 40000,
                 'source/random.bin': random.Random(9471).randbytes(CHUNK + 17),
                 'source/empty.txt': b'', 'source/unicode-\u03bb.txt': 'Research sources\n'.encode(),
                 **{f'source/small-{i:03d}.txt': f'Section {i}\n'.encode() + b'Research methods and results\n' * 4000 for i in range(80)}}
        for name, contents in files.items():
            (root / name).write_bytes(contents)
        archive = root / 'rust.mrar'
        run('create', '--output', archive, source)
        assert struct.unpack('<H', archive.read_bytes()[8:10])[0] == 1, 'Build the current /1 writer before running this check'
        decoded, codecs = read_fixture(archive, lib)
        assert decoded == files and codecs == {'store', 'zstd'}
        for version, writer in [(0, write_fixture), (1, write_grouped_fixture)]:
            # /1 helper uses one <=8 MiB group; use fewer members than the Rust multi-group fixture.
            independent_files = {k: v for k, v in files.items() if not k.startswith('source/small-') or int(k[-7:-4]) < 30}
            independent = root / f'independent-{version}.mrar'
            writer(independent, independent_files, lib)
            assert read_fixture(independent, lib)[0] == independent_files
            run('test', independent)
            destination = root / f'decoded-{version}'
            run('extract', independent, '--destination', destination)
            assert {p.relative_to(destination).as_posix(): p.read_bytes()
                    for p in destination.rglob('*') if p.is_file()} == independent_files
        report = {'format': 'MRAR/0 + MRAR/1 experimental', 'rust_writer_to_independent_reader': 'passed',
                  'independent_writer_to_rust_test_and_extract': 'passed', 'codecs_checked': sorted(codecs),
                  'cases': ['legacy /0 decoding', 'grouped /1', 'multi-group boundary', 'multi-block', 'stored random bytes', 'compressed text', 'empty file', 'UTF-8 name'],
                  'files': len(files), 'bytes': sum(map(len, files.values())),
                  'cli_sha256': sha(cli.read_bytes()),
                  'scope': 'Independent container implementation; both implementations use Zstandard. QA only; not a security audit.'}
        args.output.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
