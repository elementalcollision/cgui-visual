import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TrivyModal, DoctorModal } from './modals';
import { getTheme } from './theme';

const t = getTheme(true);

describe('TrivyModal', () => {
  it('lists every fixture finding when no filter is active', async () => {
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    // Wait for the api fallback fixture to populate.
    await screen.findByText(/CVE-2024-21626/);
    // Twelve fixture findings — at least the two CRITICAL CVEs and several HIGHs.
    expect(screen.getByText('CVE-2024-21626')).toBeInTheDocument();
    expect(screen.getByText('CVE-2025-1138')).toBeInTheDocument();
    expect(screen.getByText('CVE-2024-3094')).toBeInTheDocument();
    expect(screen.getByText('CVE-2024-7264')).toBeInTheDocument();
  });

  it('filters by severity when a severity pill is clicked', async () => {
    const user = userEvent.setup();
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    await screen.findByText(/CVE-2024-21626/);
    // Click the CRITICAL pill (text contains the count, e.g. "CRITICAL 2").
    const critPill = screen.getByRole('button', { name: /CRITICAL/i });
    await user.click(critPill);
    expect(screen.getByText('CVE-2024-21626')).toBeInTheDocument();
    expect(screen.getByText('CVE-2025-1138')).toBeInTheDocument();
    // A HIGH-only CVE should be hidden under the CRITICAL filter.
    expect(screen.queryByText('CVE-2024-3094')).not.toBeInTheDocument();
  });

  it('filters by free-text search', async () => {
    const user = userEvent.setup();
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    await screen.findByText(/CVE-2024-21626/);
    const search = screen.getByPlaceholderText(/Search CVE/);
    await user.type(search, 'openssh');
    expect(screen.getByText('CVE-2024-6387')).toBeInTheDocument();
    expect(screen.queryByText('CVE-2024-21626')).not.toBeInTheDocument();
  });
});

describe('DoctorModal', () => {
  it('renders the fixture summary line', async () => {
    render(<DoctorModal t={t} onClose={() => {}} />);
    await screen.findByText(/active profile/);
    // 6 ok + 2 warn + 0 fail in fixtures
    expect(screen.getByText(/6 passed · 2 warnings · 0 failures/)).toBeInTheDocument();
  });
});
