# Product identity

- Product: **MaxRAR**.
- Publisher: **Matily**, linked to https://www.matily.org.
- Desktop application: `MaxRAR.app`; native executable: `maxrar`.
- Automation executable: `maxrar-cli`.
- Experimental archive format: **MRAR**, extension `.mrar`.

The canonical application icon is `design/app-icon.svg`. Desktop bundle icons
and interface identity assets derive from that source. Interface icon provenance
is recorded in `public/icons/manifest.json`; preserve the associated license notices.

Internal Rust packages and private IPC identifiers retain their `wonrar` names.
This does not change the product's displayed name. New archives use experimental
[MRAR/1](MRAR-1-SPEC.md); readers also support [MRAR/0](MRAR-0-SPEC.md).

Describe compression performance with the exact inputs, settings and metrics in
recorded measurements with their limitations.
No universal compression superiority or stable MRAR interchange support is claimed.
