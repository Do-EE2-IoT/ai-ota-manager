#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import urlparse


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def model_path_from_url(repo_root: Path, url: str):
    parsed = urlparse(url)
    raw_prefix = "/Do-EE2-IoT/ai-ota-manager/main/"
    if parsed.netloc == "raw.githubusercontent.com" and parsed.path.startswith(raw_prefix):
        rel = parsed.path[len(raw_prefix):]
        return repo_root / rel
    return None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Refresh sha256 fields in an HC manifest from local config/model files."
    )
    parser.add_argument("hc_dir", help="HC directory, for example hc/24-95-07-e0-81-96")
    args = parser.parse_args()

    repo_root = Path.cwd()
    hc_dir = Path(args.hc_dir)
    manifest_path = hc_dir / "manifest.json"

    with manifest_path.open("r", encoding="utf-8") as f:
        manifest = json.load(f)

    config = manifest.get("config")
    if config and config.get("file"):
        config_path = hc_dir / config["file"]
        config["sha256"] = sha256_file(config_path)

    for key in ("detect", "verify"):
        item = manifest.get(key)
        if not item or not item.get("url"):
            continue
        local_model = model_path_from_url(repo_root, item["url"])
        if local_model and local_model.exists():
            item["sha256"] = sha256_file(local_model)

    with manifest_path.open("w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    print(f"Updated SHA256 in {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
