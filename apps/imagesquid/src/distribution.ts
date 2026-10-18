/**
 * The ImageSquid distribution definition: brand and vocabulary passed to FileSquid
 * components. The id and displayed name come from the distribution manifest and the logo is
 * the official mark in `public/` (IS-S-009). ImageSquid adds views, never backend code
 * (ImageSquid U-06).
 */
import type { Distribution } from "@filesquid/ui";
import manifest from "../distribution.json";
import labels from "./theme/labels.en.json";

/**
 * ImageSquid brand and photo-album vocabulary. The mark has its own dark tile, so one file
 * serves both themes.
 */
export const imagesquid: Distribution = {
  id: manifest.id,
  productName: manifest.productName,
  logo: { src: "/imagesquid-logo.svg", alt: `${manifest.productName} logo` },
  labels,
};
