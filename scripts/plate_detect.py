#!/usr/bin/env python
"""YOLO-based license plate detection only (no OCR)."""
import argparse
import json
import os
import sys
import time

import numpy as np
from PIL import Image

try:
    from ultralytics import YOLO
except Exception as exc:
    print(json.dumps({"detected": False, "error": f"import_error: {exc}"}))
    sys.exit(1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--yolo-model", default=os.getenv("PLATE_YOLO_MODEL", "models/plate_yolo.pt"))
    parser.add_argument("--min-conf", type=float, default=0.35)
    parser.add_argument("--device", default=os.getenv("PLATE_DEVICE", "0"))
    parser.add_argument("--output-crop", help="Save cropped plate image to this path")
    args = parser.parse_args()

    started = time.time()

    if not os.path.exists(args.image):
        print(json.dumps({"detected": False, "error": "image_not_found"}))
        sys.exit(2)

    image = Image.open(args.image).convert("RGB")
    image_np = np.array(image)

    # Auto device selection
    device = args.device
    if device == "auto":
        try:
            import torch
            device = "0" if torch.cuda.is_available() else "cpu"
        except:
            device = "cpu"

    try:
        model = YOLO(args.yolo_model)
    except Exception as exc:
        print(json.dumps({"detected": False, "error": f"yolo_load_error: {exc}"}))
        sys.exit(3)

    # Run detection
    results = model.predict(source=image_np, conf=args.min_conf, device=device, verbose=False)

    if not results or results[0].boxes is None or len(results[0].boxes) == 0:
        elapsed_ms = int((time.time() - started) * 1000)
        print(json.dumps({"detected": False, "confidence": 0.0, "elapsed_ms": elapsed_ms}))
        return

    boxes = results[0].boxes
    best_idx = int(np.argmax(boxes.conf.cpu().numpy()))
    box = boxes[best_idx]
    x1, y1, x2, y2 = [int(max(0, v)) for v in box.xyxy[0].cpu().numpy().tolist()]
    conf = float(box.conf[0].cpu().numpy().item())

    # Crop plate region
    crop_path = None
    if args.output_crop:
        plate_img = image.crop((x1, y1, x2, y2))
        plate_img.save(args.output_crop)
        crop_path = args.output_crop

    elapsed_ms = int((time.time() - started) * 1000)
    result = {
        "detected": True,
        "confidence": conf,
        "bbox": [x1, y1, x2, y2],
        "elapsed_ms": elapsed_ms,
    }
    if crop_path:
        result["crop_path"] = crop_path

    print(json.dumps(result, ensure_ascii=True))


if __name__ == "__main__":
    main()
