/** Lucide-style closed folder (sidebar). */
export const FOLDER_PATH_SIDEBAR_CLOSED =
  "M20 20 \
a2 2 0 0 0 2-2 \
V8 \
a2 2 0 0 0-2-2 \
h-7.9 \
a2 2 0 0 1-1.69-.9 \
L9.6 3.9 \
A2 2 0 0 0 7.93 3 \
H4 \
a2 2 0 0 0-2 2 \
v13 \
a2 2 0 0 0 2 2 \
Z";

/** Lucide-style open folder (sidebar expanded). */
export const FOLDER_PATH_SIDEBAR_OPEN =
  "m6 14 \
1.5-2.9 \
A2 2 0 0 1 9.24 10 \
H20 \
a2 2 0 0 1 1.94 2.5 \
l-1.55 6 \
a2 2 0 0 1-1.94 1.5 \
H4 \
a2 2 0 0 1-2-2 \
V5 \
a2 2 0 0 1 2-2 \
h4 \
a2 2 0 0 1 1.69.9 \
l2.05 2.05 \
a2 2 0 0 0 1.41.58 \
H20 \
a2 2 0 0 1 2 2 \
v2";

/** Feather-style closed folder (collection panel folder rows). */
export const FOLDER_PATH_PANEL_CLOSED =
  "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z";

export type FolderGlyphVariant = "sidebar-closed" | "sidebar-open" | "panel-closed";

export function folderPathForVariant(variant: FolderGlyphVariant): string {
  switch (variant) {
    case "sidebar-open":
      return FOLDER_PATH_SIDEBAR_OPEN;
    case "panel-closed":
      return FOLDER_PATH_PANEL_CLOSED;
    default:
      return FOLDER_PATH_SIDEBAR_CLOSED;
  }
}
