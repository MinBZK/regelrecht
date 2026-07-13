import { describe, it, expect, vi, beforeEach } from 'vitest';

// Route the composable's network leg through a controllable spy. The
// factory closes over `apiFetch` so it survives `vi.resetModules()`. Only
// `apiFetch` is stubbed - useTasks.js is the only file under test that
// imports from '@regelrecht/frontend-shared', so the other package exports
// (useAuth, useColorScheme, ...) don't need stubs here.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a) => apiFetch(...a) }));

describe('useTasks', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
  });

  it('laadt open taken en gebruikt open_count van de server', async () => {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks: [{ id: 't1', task_type: 'job_review', title: 'x' }], open_count: 5 }),
    });
    const { useTasks } = await import('./useTasks.js');
    const { tasks, openCount, refresh } = useTasks();
    await refresh();
    expect(apiFetch).toHaveBeenCalledWith('/api/tasks', expect.anything());
    expect(tasks.value).toHaveLength(1);
    expect(openCount.value).toBe(5);
  });

  it('houdt stale taken vast bij een poll-fout', async () => {
    apiFetch.mockResolvedValueOnce({
      status: 200,
      json: async () => ({ tasks: [{ id: 't1' }], open_count: 1 }),
    });
    const { useTasks } = await import('./useTasks.js');
    const { tasks, refresh } = useTasks();
    await refresh();
    apiFetch.mockRejectedValueOnce(new Error('netwerk'));
    await refresh();
    expect(tasks.value).toHaveLength(1);
  });

  it('resolve roept het resolve-endpoint en herlaadt', async () => {
    apiFetch.mockResolvedValue({ status: 200, json: async () => ({ tasks: [], open_count: 0 }) });
    const { useTasks } = await import('./useTasks.js');
    const { resolveTask } = useTasks();
    await resolveTask('t1', 'dismissed');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t1/resolve',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('requestEnrich meldt alreadyRunning bij 409 en tooMany bij 429', async () => {
    apiFetch.mockResolvedValueOnce({ status: 409 });
    const { useTasks } = await import('./useTasks.js');
    const { requestEnrich } = useTasks();
    const r1 = await requestEnrich('traject-abcd1234', 'test_wet');
    expect(r1.alreadyRunning).toBe(true);
    apiFetch.mockResolvedValueOnce({ status: 429 });
    const r2 = await requestEnrich('traject-abcd1234', 'test_wet');
    expect(r2.tooMany).toBe(true);
  });
});
