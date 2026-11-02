/**
 * The error card of a refusal (UI-1.2, UI-S-029, mockup 48): a box outlined in `--fs-error` with
 * the error icon, the title and the reason, and **Dismiss** below it. The card is a `role="alert"`
 * so a screen reader announces it; Dismiss removes it and changes nothing else. The reason always
 * says that nothing was changed, as the refusals of the API leave everything as it was.
 */
import type { LabelTable } from "../theme/labels";
import { icon } from "./icons";
import { actionButton, node } from "./library-parts";

/**
 * Build one refusal card.
 *
 * @param labels The active label table.
 * @param title What was refused, for example `Not uploaded: photo.jpg`.
 * @param reason Why, and what to do next.
 * @param onDismiss Called after the card is removed.
 * @returns The card; add it where the refusal belongs.
 */
export function refusalCard(
  labels: LabelTable,
  title: string,
  reason: string,
  onDismiss?: () => void,
): HTMLElement {
  const card = node("div", "refusal");
  card.setAttribute("role", "alert");
  const box = node("div", "note-box error");
  box.append(icon("error"), node("p", "title", title), node("p", "reason", reason));
  const dismiss = actionButton("dismiss", labels.fileDismiss);
  dismiss.addEventListener("click", () => {
    card.remove();
    onDismiss?.();
  });
  card.append(box, dismiss);
  return card;
}
