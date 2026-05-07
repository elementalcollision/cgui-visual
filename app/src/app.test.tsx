// Integration test of the full <App />. Exercises tab switching, modal
// open/close, and keyboard handling against the in-jsdom fixture fallback.

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from './App';

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

  it('Pull image button opens the pull modal', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('mlperf-inference-llama2');
    await user.click(screen.getByRole('button', { name: /Pull image/ }));
    expect(await screen.findByText(/Pulling image|Pulled|Pull failed/)).toBeInTheDocument();
  });
});
