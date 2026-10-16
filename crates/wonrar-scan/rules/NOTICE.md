# WonRaR baseline rules

Version: 2026.10.14.1. Author: WonRaR contributors.

These five baseline rules were authored for WonRaR. No third-party rule collection was copied. These original rules are licensed under Apache License 2.0; see the repository LICENSE and NOTICE files.

The benign WonRaR sentinel verifies integration. The EICAR rule recognizes the public antivirus test marker and labels it explicitly as a non-malicious test. The remaining rules look for co-occurring download/execution or deserialization/process-call text. Such combinations can occur in legitimate software and documentation; matches require review and are not confirmed malware labels.

Rule matching is implemented by YARA-X 1.21.0, independently licensed BSD-3-Clause. No VirusTotal service, online query, cloud upload or remote rules download is used.
