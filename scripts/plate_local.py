#!/usr/bin/env python
import argparse
import json
import os
import sys
import time

import numpy as np
from PIL import Image

try:
    from ultralytics import YOLO
    from paddleocr import PaddleOCR
    import torch
except Exception as exc:
    print(json.dumps({"plate": None, "error": f"import_error: {exc}"}))
    sys.exit(1)


def detect_plate(model, image_np, min_conf, device):
    results = model.predict(source=image_np, conf=min_conf, device=device, verbose=False)
    if not results or results[0].boxes is None or len(results[0].boxes) == 0:
        return None
    boxes = results[0].boxes
    best_idx = int(np.argmax(boxes.conf.cpu().numpy()))
    box = boxes[best_idx]
    x1, y1, x2, y2 = box.xyxy[0].cpu().numpy().tolist()
    conf = float(box.conf[0].cpu().numpy().item())
    return [x1, y1, x2, y2], conf


def ocr_plate(ocr, plate_img):
    result = ocr.ocr(plate_img, cls=False)
    if not result:
        return None, 0.0
    best_text = None
    best_conf = 0.0
    for line in result:
        for box, (text, conf) in line:
            if conf > best_conf:
                best_conf = conf
                best_text = text
    return best_text, float(best_conf)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--yolo-model", default=os.getenv("PLATE_YOLO_MODEL", "models/plate_yolo.pt"))
    parser.add_argument("--min-conf", type=float, default=0.35)
    parser.add_argument("--device", default=os.getenv("PLATE_DEVICE", "0"))
    parser.add_argument("--ocr-lang", default=os.getenv("PLATE_OCR_LANG", "japan"))
    args = parser.parse_args()

    started = time.time()

    if not os.path.exists(args.image):
        print(json.dumps({"plate": None, "error": "image_not_found"}))
        sys.exit(2)

    image = Image.open(args.image).convert("RGB")
    image_np = np.array(image)

    device = args.device
    if device == "auto":
        device = "0" if torch.cuda.is_available() else "cpu"

    try:
        model = YOLO(args.yolo_model)
    except Exception as exc:
        print(json.dumps({"plate": None, "error": f"yolo_load_error: {exc}"}))
        sys.exit(3)

    det = detect_plate(model, image_np, args.min_conf, device)
    if det is None:
        print(json.dumps({"plate": None, "confidence": 0.0}))
        return

    bbox, det_conf = det
    x1, y1, x2, y2 = [int(max(0, v)) for v in bbox]
    plate_img = image.crop((x1, y1, x2, y2))

    use_gpu = device not in ("cpu", "-1")
    ocr = PaddleOCR(lang=args.ocr_lang, use_gpu=use_gpu, show_log=False)
    text, ocr_conf = ocr_plate(ocr, np.array(plate_img))

    elapsed_ms = int((time.time() - started) * 1000)
    print(
        json.dumps(
            {
                "plate": text,
                "confidence": float(det_conf),
                "ocr_confidence": float(ocr_conf),
                "bbox": [x1, y1, x2, y2],
                "elapsed_ms": elapsed_ms,
            },
            ensure_ascii=True,
        )
    )


if __name__ == "__main__":
    main()
