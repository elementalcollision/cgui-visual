// Multi-step container actions shared by the detail modal (and any row action).

import { api } from './api';
import { askText } from './dialogs';
import { toast, withToast } from './toast';
import { validateImageRef } from './validate';

/** Default image reference offered when committing a container. */
export function defaultCommitRef(containerName: string): string {
  const base = containerName.toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^[^a-z0-9]+|[^a-z0-9]+$/g, '');
  return `${base || 'container'}:latest`;
}

/**
 * Ask for an image reference (re-prompting with the reason until it is valid
 * or the user cancels), commit the container to it, toast the result and
 * call `onCommitted` so the Images list can refresh. Needs container 1.6+;
 * callers gate the control with `useCliAtLeast('1.6.0')`.
 */
export async function commitContainerFlow(
  c: { id: string; name: string },
  onCommitted?: () => void,
): Promise<void> {
  let ref = defaultCommitRef(c.name);
  let problem: string | null = null;
  for (;;) {
    const answer = await askText(
      `${problem ? `${problem}\n\n` : ''}Image reference for the committed ${c.name}:`,
      ref,
    );
    if (answer === null) return;
    ref = answer.trim();
    problem = validateImageRef(ref);
    if (!problem) break;
  }
  try {
    await withToast(`commit ${c.name}`, api.commitContainer(c.id, ref));
  } catch {
    return; // withToast already reported the failure
  }
  toast(`Committed ${c.name} to ${ref}`, 'info');
  onCommitted?.();
}
