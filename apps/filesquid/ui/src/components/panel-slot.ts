/**
 * What a panel in the shell's `panel` slot may ask of the shell (UI-S-031, UI-S-032), kept apart
 * from the shell so a panel needs no import of it (panels never depend on each other or on the
 * frame, ADR 028).
 */

/**
 * The attribute a slotted panel carries to ask for the wide panel region (360 px, mockup 38,
 * the Logs entry detail) in place of the file panel's 320 px.
 */
export const PANEL_WIDE = "data-panel-wide";
