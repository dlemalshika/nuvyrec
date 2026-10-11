"""Loopback RTMP integration checks with generated pixels/audio only.

Requires the release stream_fixture example and bundled FFmpeg tools. Never
reads the desktop, opens a webcam, or contacts a platform/account. Creates a new
artifact directory; all child processes and sockets are owned by this test.
"""
import argparse
import json
from pathlib import Path
import socket
import ssl
import subprocess
import threading
import time
import uuid


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def probe(ffprobe, path):
    result = subprocess.run(
        [str(ffprobe), '-v', 'error', '-show_streams', '-show_format',
         '-of', 'json', str(path)], capture_output=True, text=True, timeout=15,
        check=True)
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path,
                        default=Path('target/gpu-overlays/release/examples/stream_fixture.exe'))
    parser.add_argument('--ffmpeg', type=Path, required=True)
    parser.add_argument('--tls-test', action='store_true',
                        help='Also reject a generated self-signed TLS peer (requires cryptography).')
    args = parser.parse_args()
    ffprobe = args.ffmpeg.with_name('ffprobe.exe')
    directory = Path('artifacts') / ('streaming-fixture-' + uuid.uuid4().hex)
    directory.mkdir(parents=True)
    results = []
    scenarios = ['three-targets', 'failure-isolation', 'reconnect']
    if args.tls_test:
        scenarios.append('certificate-rejection')
    for scenario in scenarios:
        root = directory / scenario
        root.mkdir()
        ports = [free_port() for _ in range(3)]
        children = []
        receiver_logs = []
        stalled = None
        tls = None
        accepted = []
        tls_rejected, tls_accepted = [], []

        def receiver(index, suffix=''):
            path = root / f'received-{index}{suffix}.flv'
            log = open(root / f'receiver-{index}{suffix}.log', 'wb')
            receiver_logs.append(log)
            child = subprocess.Popen(
                [str(args.ffmpeg), '-nostdin', '-hide_banner', '-loglevel', 'error',
                 '-analyzeduration', '500000', '-probesize', '32768',
                 '-listen', '1', '-i', f'rtmp://127.0.0.1:{ports[index]}/live/fixture',
                 '-c', 'copy', '-flush_packets', '1', '-f', 'flv', str(path)],
                stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            children.append(child)
            return child

        try:
            receiver(0)
            second = receiver(1)
            if scenario not in ('failure-isolation', 'certificate-rejection'):
                receiver(2)
            else:
                # A TCP peer accepts RTMP but never completes the handshake.
                # This tests the timeout/stop callback without external IO.
                stalled = socket.socket()
                stalled.bind(('127.0.0.1', ports[2]))
                stalled.listen(4)
                stalled.settimeout(12)

                tls = None
                if scenario == 'certificate-rejection':
                    from cryptography import x509
                    from cryptography.hazmat.primitives import hashes, serialization
                    from cryptography.hazmat.primitives.asymmetric import rsa
                    from cryptography.x509.oid import NameOID
                    import datetime
                    import ipaddress
                    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
                    name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, 'localhost')])
                    now = datetime.datetime.now(datetime.timezone.utc)
                    certificate = (x509.CertificateBuilder().subject_name(name).issuer_name(name)
                                   .public_key(key.public_key()).serial_number(x509.random_serial_number())
                                   .not_valid_before(now - datetime.timedelta(minutes=1))
                                   .not_valid_after(now + datetime.timedelta(days=1))
                                   .add_extension(x509.SubjectAlternativeName([
                                       x509.DNSName('localhost'), x509.IPAddress(ipaddress.ip_address('127.0.0.1'))]), critical=False)
                                   .sign(key, hashes.SHA256()))
                    cert_path, key_path = root / 'test-only.crt', root / 'test-only.key'
                    cert_path.write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
                    key_path.write_bytes(key.private_bytes(serialization.Encoding.PEM,
                                                          serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
                    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                    tls.load_cert_chain(cert_path, key_path)

                def accept_stalled():
                    try:
                        connection, _ = stalled.accept()
                        accepted.append(connection)
                        if tls:
                            try:
                                connection.settimeout(7)
                                secured = tls.wrap_socket(connection, server_side=True)
                                # FFmpeg/GnuTLS verifies AFTER the TLS handshake.
                                # A successful SSL handshake alone proves nothing:
                                # an untrusted peer must receive no RTMP payload.
                                data = secured.recv(1)
                                if data:
                                    tls_accepted.append(data.hex())
                                else:
                                    tls_rejected.append(True)
                                secured.close()
                            except ssl.SSLError:
                                tls_rejected.append(True)
                    except OSError:
                        pass
                threading.Thread(target=accept_stalled, daemon=True).start()
            time.sleep(.6)
            started = time.monotonic()
            fixture = subprocess.Popen(
                [str(args.fixture), str(root / 'local.mkv'), *map(str, ports),
                 *(['--tls-last'] if tls else [])],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            children.append(fixture)
            if scenario == 'reconnect':
                time.sleep(2.6)
                second.terminate()
                second.wait(timeout=5)
                time.sleep(.5)
                receiver(1, '-reconnected')
            stdout, stderr = fixture.communicate(timeout=22)
            (root / 'fixture.log').write_text(stdout + stderr, encoding='utf-8')
            assert fixture.returncode == 0, stdout + stderr
            assert 'fixture' not in stderr, 'FFmpeg network log leaked the fixture stream key'
            assert time.monotonic() - started < 17, 'network shutdown stalled recording'
            unavailable = scenario in ('failure-isolation', 'certificate-rejection')
            assert 'tiktok stopped packets=0' in stdout if unavailable else 'tiktok stopped packets=' in stdout
            if scenario == 'certificate-rejection':
                assert tls_rejected and not tls_accepted, 'Self-signed TLS certificate was accepted'
            local = probe(ffprobe, root / 'local.mkv')
            assert len(local['streams']) == 3, 'local independent audio track lost'
            assert float(local['format']['duration']) >= 9.9, 'local recording truncated'
            for child in children:
                if child is fixture:
                    continue
                try:
                    child.wait(timeout=6)
                except subprocess.TimeoutExpired:
                    child.terminate()
                    child.wait(timeout=5)
            paths = [root / 'received-0.flv', root / 'received-1.flv']
            if not unavailable:
                paths.append(root / 'received-2.flv')
            if scenario == 'reconnect':
                paths.append(root / 'received-1-reconnected.flv')
            for path in paths:
                info = probe(ffprobe, path)
                assert [s['codec_name'] for s in info['streams']] == ['h264', 'aac'], path
                duration = float(info['format']['duration'])
                assert duration >= (1 if scenario == 'reconnect' else 5), (path, duration)
                assert duration < 9, 'Stop live failed to stop publishing while recording continued'
                subprocess.run([str(args.ffmpeg), '-nostdin', '-v', 'error', '-i', str(path),
                                '-f', 'null', '-'], capture_output=True, timeout=20, check=True)
            results.append({'scenario': scenario, 'passed': True,
                            'receivers': len(paths), 'fixture_status': stdout.strip()})
        finally:
            for child in children:
                if child.poll() is None:
                    child.terminate()
                    child.wait(timeout=5)
            if stalled:
                stalled.close()
            for connection in accepted:
                connection.close()
            for log in receiver_logs:
                log.close()
    (directory / 'results.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
    print(json.dumps({'passed': True, 'directory': str(directory), 'results': results}, indent=2))


if __name__ == '__main__':
    main()
