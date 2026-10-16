# README media

These files are intentionally included in the public repository. Private design
captures remain excluded by the root `.gitignore`.

| Asset | Source |
| --- | --- |
| `maxrar-hero.gif`, `maxrar-hero.png` | Original workflow illustration rendered by `scripts/generate-readme-hero.py` |
| `archive-inspector.png` | Sample-data interface capture, `design/screenshots/maxrar-simple-inspector-v1.png` |
| `extract-files.png` | Interface capture, `design/screenshots/maxrar-extraction-aligned-v2.png` |
| `local-scan.png` | Cropped sample-data preview, `design/screenshots/maxrar-selection-scan-v1.png`; development controls excluded |

The hero uses the existing desktop icon in `src-tauri/icons/icon.png`, derived
from the canonical `design/app-icon.svg`. It is an illustrative workflow, not
a recording or a speed benchmark. Its static PNG is available in the README.
The interface captures depict development screens and benign sample data;
they do not establish release validation on every supported platform.

To regenerate the hero, install Pillow in a development environment and run:

```sh
python3 scripts/generate-readme-hero.py
```

The generator currently uses macOS system Arial fonts. On other hosts, adjust
the font lookup to an equivalent installed font. It does not require or change
the application's production dependencies. Interface icon licensing remains
documented in `public/icons/NOTICE` and `design/icons/fluent/NOTICE`.

`buttons/*.svg` are original static README navigation badges. They are local assets so the button row does not depend on a third-party badge service or unpublished GitHub release metadata. Download links to the availability section until installers are published.
