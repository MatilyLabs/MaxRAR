# Benign interoperability fixtures

`python-pax.tar.gz` and `python-gnu.tar.gz` were generated using Python's standard
`tarfile` and `gzip` libraries, with PAX_FORMAT and GNU_FORMAT respectively. Each
contains one ordinary TeX source with a path longer than 100 bytes and one symlink
to `../../outside` that must be blocked. They contain no executable payloads.
All files use fixed timestamps; gzip's original-filename field is empty.

`python-single-source.gz` uses `gzip.compress` on a single short UTF-8 TeX source,
without a TAR wrapper. Tests copy it to an extensionless arXiv-style name.

These locally generated fixtures exercise compatibility with another implementation;
they are not a broad archive compatibility corpus or actual arXiv downloads.
