/**
 * useDocumentUpload - the shared "hidden native file input + trigger" wiring for
 * the werkdocumenten upload affordance, used by both the launcher sheet and the
 * standalone page. No NLDD file-upload component exists, so the picker is a
 * hidden `<input type="file">`; this keeps that one bit of non-design-system
 * plumbing in a single place.
 *
 * @param {(file: File) => Promise<{ ok: boolean }>} uploadFn  performs the upload
 * @param {() => void} [onUploaded]  called after a successful upload (e.g. start polling)
 */
import { ref } from 'vue';

export function useDocumentUpload(uploadFn, onUploaded) {
  const fileInput = ref(null);
  // Surfaced to the consumer so a failed upload (400/413/503/network) is shown,
  // not silently swallowed when the file picker closes.
  const uploadError = ref(null);
  // Whether an "opnieuw proberen" retry makes sense. False for a permanent
  // server-side gap (e.g. the upload endpoint isn't supported yet), so the
  // consumer can drop its retry action.
  const uploadRetryable = ref(true);

  function onUpload() {
    fileInput.value?.click();
  }

  async function onFileChange(e) {
    const file = e.target.files?.[0];
    // Reset the input so re-picking the same file fires `change` again.
    e.target.value = '';
    if (!file) return;
    uploadError.value = null;
    uploadRetryable.value = true;
    const result = await uploadFn(file);
    if (result?.ok) {
      if (onUploaded) onUploaded();
    } else {
      // Set retryability before the message so a consumer watching the error
      // reads the matching value. A result without `retryable` defaults to true.
      uploadRetryable.value = result?.retryable !== false;
      uploadError.value = result?.error || 'Uploaden mislukt.';
    }
  }

  return { fileInput, uploadError, uploadRetryable, onUpload, onFileChange };
}
