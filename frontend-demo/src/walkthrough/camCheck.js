/**
 * The camera test in the recorder panel: which camera, what it sees, and
 * whether there is enough light.
 *
 * The presenter sets up background and light before a take, not after: a
 * dark or cluttered picture cannot be fixed in post. The preview shows the
 * round cut the viewer will see (a square around the face, as the pipeline
 * crops it), and a plain verdict on the brightness.
 */
import { reactive } from 'vue';

export const cam = reactive({
  devices: [],
  deviceId: '',
  testing: false,
  /** The stream the preview shows; null when the test is off. */
  stream: null,
  /** Mean luma of the picture, 0 to 255. */
  brightness: 0,
});

let timer = 0;

/** A verdict on the brightness: the key of its text. */
export function lightVerdict(brightness) {
  if (brightness < 60) return 'recorder.cam.dark';
  if (brightness > 200) return 'recorder.cam.bright';
  return 'recorder.cam.good';
}

export async function listCameras() {
  const all = (await navigator.mediaDevices?.enumerateDevices?.()) ?? [];
  cam.devices = all.filter((d) => d.kind === 'videoinput').map((d) => ({ id: d.deviceId, label: d.label || d.deviceId.slice(0, 8) }));
  if (!cam.deviceId && cam.devices.length) cam.deviceId = cam.devices[0].id;
}

/** The constraints a take records with: the chosen camera, 1080p. */
export function camConstraints() {
  return {
    ...(cam.deviceId ? { deviceId: { exact: cam.deviceId } } : {}),
    width: { ideal: 1920 },
    height: { ideal: 1080 },
    frameRate: 30,
  };
}

/** Mean luma of an RGBA pixel buffer. */
export function meanLuma(data) {
  let sum = 0;
  for (let i = 0; i < data.length; i += 4) sum += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
  return data.length ? Math.round(sum / (data.length / 4)) : 0;
}

export async function startCamTest() {
  stopCamTest();
  const stream = await navigator.mediaDevices.getUserMedia({ video: camConstraints(), audio: false });
  // Labels are only filled in once a page has camera permission.
  await listCameras();
  cam.stream = stream;
  cam.testing = true;
  const video = document.createElement('video');
  video.muted = true;
  video.playsInline = true;
  video.srcObject = stream;
  await video.play().catch(() => {});
  const canvas = document.createElement('canvas');
  canvas.width = 64;
  canvas.height = 36;
  const g = canvas.getContext('2d', { willReadFrequently: true });
  timer = setInterval(() => {
    if (!video.videoWidth) return;
    g.drawImage(video, 0, 0, canvas.width, canvas.height);
    cam.brightness = meanLuma(g.getImageData(0, 0, canvas.width, canvas.height).data);
  }, 500);
}

export function stopCamTest() {
  clearInterval(timer);
  cam.stream?.getTracks().forEach((t) => t.stop());
  cam.stream = null;
  cam.testing = false;
  cam.brightness = 0;
}
