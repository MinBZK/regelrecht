"""Find the presenter's face in a webcam take and print a square crop around it.

Run in its own environment (cli.py starts it with `uv run --with
opencv-python-headless`). Samples frames across the take, detects faces with
OpenCV's bundled Haar cascade, and takes the median: the presenter sits
still, so one fixed crop is steadier than following the face around.

    python camcrop_run.py cam.mkv  ->  {"x": .., "y": .., "size": .., "found": n}
"""

import json
import statistics
import sys


def main() -> None:
    import cv2

    path = sys.argv[1]
    cap = cv2.VideoCapture(path)
    width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
    height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    cascade = cv2.CascadeClassifier(cv2.data.haarcascades + "haarcascade_frontalface_default.xml")
    boxes = []
    # MediaRecorder WebM often has no frame count; read through and sample.
    i = 0
    while True:
        ok, frame = cap.read()
        if not ok:
            break
        i += 1
        if i % 45:
            continue
        gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
        faces = cascade.detectMultiScale(gray, scaleFactor=1.1, minNeighbors=6, minSize=(height // 8, height // 8))
        if len(faces):
            boxes.append(max(faces, key=lambda f: f[2] * f[3]))
        if len(boxes) >= 40:
            break
    if not boxes:
        size = min(width, height)
        print(json.dumps({"x": (width - size) // 2, "y": (height - size) // 2, "size": size, "found": 0}))
        return
    cx = statistics.median(b[0] + b[2] / 2 for b in boxes)
    cy = statistics.median(b[1] + b[3] / 2 for b in boxes)
    fh = statistics.median(b[3] for b in boxes)
    # Head and a bit of shoulder: the face takes about 40% of the circle, and
    # sits slightly above the middle, the way a portrait is framed.
    size = int(min(width, height, fh * 2.5))
    size -= size % 2
    x = int(min(max(cx - size / 2, 0), width - size))
    y = int(min(max(cy - size * 0.45, 0), height - size))
    print(json.dumps({"x": x - x % 2, "y": y - y % 2, "size": size, "found": len(boxes)}))


if __name__ == "__main__":
    main()
