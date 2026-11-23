// Barrel for the modal dialogs. Everything the old single-file `modals.tsx`
// exported is re-exported here, so `import { X } from './modals'` is unchanged.

export { DetailModal } from './DetailModal';
export { PullModal } from './PullModal';
export { BuildModal } from './BuildModal';
export { TrivyModal } from './TrivyModal';
export { UpdateModal } from './UpdateModal';
export { DoctorModal } from './DoctorModal';
export { ImageInspectModal } from './ImageInspectModal';
export { MachineLogsModal } from './MachineLogsModal';
export { JsonInspectModal } from './JsonInspectModal';
export { CreateVolumeModal } from './CreateVolumeModal';
export { CreateNetworkModal } from './CreateNetworkModal';
export { RunImageModal } from './RunImageModal';
export { OnboardingModal } from './OnboardingModal';
export { SettingsModal } from './SettingsModal';
export { MachinesPanel } from './settings/MachinesPanel';
export { CommandPaletteModal, fuzzyScore } from './CommandPalette';
export type { CommandKind, CommandResult } from './CommandPalette';
export { HelpModal } from './HelpModal';
export { parseInspect, parseImageInspect } from './inspect';
export type { ImageLayer, ParsedImage } from './inspect';
