export interface PlannedEntry {
  entry_id: string; source_name: string; output_name: string; size: number;
  is_dir: boolean; conflict: boolean; action: string; reason?: string | null;
}
export interface ExtractionView {
  destination: string; requested_destination?: string; destination_exists?: boolean; entries: PlannedEntry[]; total_bytes: number;
}
export function destinationParts(path: string) {
  const parts = path.replace(/[\\/]+$/, '').split(/[\\/]/).filter(Boolean);
  return { name: parts.at(-1) || path || 'Choose a folder', parent: parts.at(-2) || '', full: path };
}
export function extractionSummary(plan: ExtractionView | null) {
  const entries = plan?.entries || [];
  const files = entries.filter(e => !e.is_dir);
  const conflicts = entries.filter(e => e.conflict);
  const blocked = entries.filter(e => e.action === 'blocked');
  const writing = entries.filter(e => e.action === 'extract');
  const fileCount = writing.filter(e => !e.is_dir).length;
  const skipped = files.filter(e => e.action === 'skip').length;
  const count = fileCount || writing.length;
  const unit = fileCount || !writing.length ? 'file' : 'folder';
  return { files: files.length, conflicts, blocked, writing: writing.length, fileCount, skipped,
    outcome: `${count} ${unit}${count === 1 ? '' : 's'} to extract${skipped ? ` · ${skipped} will be skipped` : ''}`,
    button: writing.length ? `Extract ${count} ${unit}${count === 1 ? '' : 's'}` : 'Nothing to extract' };
}
