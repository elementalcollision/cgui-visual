// Integration test of the full <App />. Exercises tab switching, modal
// open/close, and keyboard handling against the in-jsdom fixture fallback.

import { describe, it, expect, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from './App';
import { api } from './api';

describe('App integration', () => {
  it('initial render shows Containers tab with the fixture container list', async () => {
    render(<App />);
    // First fixture container row — unique enough to avoid the multiple
    // "Containers" matches from sidebar + heading + status bar.
    expect(await screen.findByText('mlperf-inference-llama2')).toBeInTheDocument();
  });

  it('sidebar click switches tabs', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('mlperf-inference-llama2');
    await user.click(screen.getByRole('button', { name: /Images/ }));
    // The image table renders refs from the fixture image set.
    expect(await screen.findByText('mlcommons/inference:llama2-70b')).toBeInTheDocument();
  });

  it('Doctor sidebar button opens the modal and Escape closes it', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('mlperf-inference-llama2');
    await user.click(screen.getByRole('button', { name: /Doctor/ }));
    // Modal header.
    expect(await screen.findByText('cgui doctor')).toBeInTheDocument();
    // Summary line from the fixture set.
    expect(await screen.findByText(/6 passed · 2 warnings · 0 failures/)).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByText('cgui doctor')).not.toBeInTheDocument();
  });

  it('Pull image button opens the pull modal in idle state', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('mlperf-inference-llama2');
    await user.click(screen.getByRole('button', { name: /Pull image/ }));
    // Modal opens idle now (no auto-pull on mount). The header reads
    // "Pull image" and an input field is present for the user to type
    // the reference. Pull doesn't fire until the Pull button is clicked.
    expect(await screen.findByPlaceholderText(/alpine:latest/)).toBeInTheDocument();
    // Pull button is present but disabled while the input is empty.
    const pullBtn = screen.getByRole('button', { name: /^Pull$/ });
    expect(pullBtn).toBeDisabled();
  });

  it('Reclaim space appears on running container rows, is absent on stopped ones, and calls the API', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'cleanContainers').mockResolvedValue(undefined);
    render(<App />);
    await screen.findByText('mlperf-inference-llama2');

    // Running row ('mlperf-inference-llama2', fixture id a3f8e2c1) gets the action.
    const runningRow = screen.getByText('mlperf-inference-llama2').parentElement!.parentElement!;
    const reclaimBtn = within(runningRow).getByTitle(/Reclaim space/);
    expect(reclaimBtn).toBeInTheDocument();

    // Stopped row ('storage-bench-fio', status 'exited') does not.
    const stoppedRow = screen.getByText('storage-bench-fio').parentElement!.parentElement!;
    expect(within(stoppedRow).queryByTitle(/Reclaim space/)).not.toBeInTheDocument();

    await user.click(reclaimBtn);
    expect(spy).toHaveBeenCalledWith(['a3f8e2c1']);
  });
});
