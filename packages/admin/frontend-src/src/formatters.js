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

export function jobSubtitle(job) {
  const type = job.job_type
    ? job.job_type.charAt(0).toUpperCase() + job.job_type.slice(1)
    : 'Job';
  switch (job.status) {
    case 'completed':
      return `${type} completed at ${formatDate(job.completed_at)}`;
    case 'failed': {
      const attempts = job.attempts > 1 ? ` after ${job.attempts} attempts` : '';
      return `${type} failed${attempts} at ${formatDate(job.completed_at)}`;
    }
    case 'processing':
      return `${type} started at ${formatDate(job.started_at)}`;
    default:
      return `${type} created at ${formatDate(job.created_at)}`;
  }
}
