#!/usr/bin/env python3
import argparse
import csv
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Normalize macOS app inventory output into the schema expected by the automation probe."
    )
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with input_path.open("r", encoding="utf-8") as source:
        reader = csv.DictReader(source, delimiter="\t")
        rows = list(reader)

    with output_path.open("w", encoding="utf-8", newline="") as target:
        fieldnames = ["kind", "name", "identifier_or_path", "path"]
        writer = csv.DictWriter(target, fieldnames=fieldnames, delimiter="\t")
        writer.writeheader()
        for row in rows:
            bundle_id = row.get("bundle id", "").strip()
            writer.writerow(
                {
                    "kind": "app",
                    "name": row.get("display name", "").strip(),
                    "identifier_or_path": bundle_id,
                    "path": row.get("path", "").strip(),
                }
            )


if __name__ == "__main__":
    main()

