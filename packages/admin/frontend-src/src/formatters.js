import { DATE_FORMATTER } from './constants.js';

export function formatDate(value) {
  if (value === null || value === undefined) return null;
  const date = new Date(value);
  if (isNaN(date.getTime())) return String(value);
  return DATE_FORMATTER.format(date);
}

export function formatCoverageScore(value) {
  if (value === null || value === undefined) return null;
  const num = Number(value);
  if (Number.isFinite(num)) return `${Math.round(num * 100)}%`;
  return String(value);
}

export function truncateUuid(value) {
  const str = String(value);
  return str.length > 8 ? str.substring(0, 8) : str;
}

export function truncateError(error, maxLen = 80) {
  if (!error) return null;
  return error.length > maxLen ? error.substring(0, maxLen) + '\u2026' : error;
}
