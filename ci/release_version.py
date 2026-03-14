#!/usr/bin/env python3
import tomllib
from pathlib import Path

manifest = tomllib.loads(Path("Cargo.toml").read_text())
package = manifest.get("package", {})
print(package["version"])
