/**
 * Public entry point of `@filesquid/ui`: API client, custom elements, theme helpers.
 * Distributions import from here and add their own views, tokens and labels.
 */
import { FsAppShell } from "./components/fs-app-shell";
import { FsBreadcrumb } from "./components/fs-breadcrumb";
import { FsDestinationPicker } from "./components/fs-destination-picker";
import { FsNewMenu } from "./components/fs-new-menu";
import { FsDriveView } from "./components/fs-drive-view";
import { FsFileBrowser } from "./components/fs-file-browser";
import { FsFilePreview } from "./components/fs-file-preview";
import { FsDetailsPanel } from "./components/fs-details-panel";
import { FsMetadataPanel } from "./components/fs-metadata-panel";
import { FsFilePanel } from "./components/fs-file-panel";
import { FsVersionsPanel } from "./components/fs-versions-panel";
import { FsHostIndicator } from "./components/fs-host-indicator";
import { FsLibraryDetail } from "./components/fs-library-detail";
import { FsLibraryRepair } from "./components/fs-library-repair";
import { FsLibraryStart } from "./components/fs-library-start";
import { FsMonitor } from "./components/fs-monitor";
import { FsNavTree } from "./components/fs-nav-tree";
import { FsOcflFindings } from "./components/fs-ocfl-findings";
import { FsNavigationView } from "./components/fs-navigation-view";
import { FsSettings } from "./components/fs-settings";
import { FsStates } from "./components/fs-states";
import { FsThemeControl } from "./components/fs-theme-control";

export { FsAppShell } from "./components/fs-app-shell";
export {
  NEW_EVENT,
  type ShellClient,
  type ShellOptions,
  type StorageSummary,
} from "./components/fs-app-shell";
export { FsThemeControl, type ThemeControlOptions } from "./components/fs-theme-control";
export {
  FsHostIndicator,
  HOST_EVENT,
  type HostClient,
  type HostIndicatorOptions,
} from "./components/fs-host-indicator";
export { icon, type IconName } from "./components/icons";
export { CONTROL_STYLE } from "./components/control-style";
export { CONTRACT_VERSION } from "./api/contract-version";
export {
  HOST_TYPES,
  describeHost,
  parseHostDescriptor,
  type HostDescriptor,
  type HostType,
  type HostView,
  type ServiceView,
} from "./host/descriptor";
export { FsMonitor, formatGiB, processCells } from "./components/fs-monitor";
export {
  FsOcflFindings,
  type FindingsClient,
  type FindingsOptions,
} from "./components/fs-ocfl-findings";
export { FsStates, RETRY_EVENT, type StatesOptions } from "./components/fs-states";
export { failureState, type ViewState, type ViewStateKind } from "./components/view-state";
export { FsNavTree, type NavTreeOptions } from "./components/fs-nav-tree";
export { FsBreadcrumb, type BreadcrumbOptions } from "./components/fs-breadcrumb";
export { FsFilePreview, type FilePreviewOptions } from "./components/fs-file-preview";
export {
  FsFileBrowser,
  type FileBrowserOptions,
  type FilePanel,
  type PanelFactory,
} from "./components/fs-file-browser";
export {
  FsDetailsPanel,
  type DetailsPanelOptions,
  type DetailsSource,
} from "./components/fs-details-panel";
export {
  FILE_PANEL_CLOSE_EVENT,
  FsFilePanel,
  type FilePanelOptions,
} from "./components/fs-file-panel";
export {
  FsMetadataPanel,
  METADATA_OPERATIONS,
  type MetadataPanelOptions,
} from "./components/fs-metadata-panel";
export {
  MAX_DIALOG_CANDIDATES,
  METADATA_DIALOG_STYLE,
  buildMetadataDialog,
  chooseProperty,
  keepTabInside,
  renderCandidates,
  type CandidateDescriber,
  type MetadataDialog,
  type MetadataDialogMode,
  type MetadataDialogParams,
} from "./components/metadata-dialog";
export {
  ClientMetadataSource,
  metadataFileId,
  type MetadataClient,
  type MetadataSource,
} from "./files/metadata-source";
export {
  DCTERMS_FILE_TERMS,
  MAX_STATEMENT_ROWS,
  MAX_STATEMENT_VALUE_BYTES,
  MAX_TERM_MATCHES,
  MAX_TERM_NAME_BYTES,
  TECHNICAL_PREFIXES,
  customKeyProblem,
  customNamespaces,
  dctermsDescription,
  descriptiveStatements,
  groupCandidates,
  matchingTerms,
  metadataCountText,
  statementRows,
  validTermName,
  valueProblem,
  type CandidateGroups,
  type FileStatements,
  type MetadataCommit,
  type MetadataRow,
  type MetadataStatement,
  type NamespaceRegistration,
  type Namespaces,
  type RegisteredCandidate,
  type StatementSource,
  type TermCandidate,
  type TermCandidates,
} from "./files/metadata";
export {
  FsVersionsPanel,
  VERSIONS_CLOSE_EVENT,
  type VersionsPanelOptions,
  type VersionsSource,
} from "./components/fs-versions-panel";
export {
  MAX_PROPERTY_ROWS,
  PROPERTY_TABLE_STYLE,
  PropertyTable,
  type PropertyTableOptions,
  type PropertyTableState,
} from "./components/property-table";
export { MAX_LABEL_BYTES, labelProblem } from "./components/version-restore-dialog";
export {
  buildLabelDialog,
  labelChange,
  type LabelChange,
  type LabelDialogParams,
} from "./components/version-label-dialog";
export { saveFileToDevice } from "./components/save-file";
export {
  ClientDownloadSource,
  DESKTOP_ANSWER_CAP,
  MAX_DOWNLOAD_READS,
  RANGE_CHUNK,
  type DownloadClient,
  type DownloadSource,
} from "./files/download-source";
export {
  ClientHistorySource,
  type FileHistory,
  type FileHistoryEntry,
  type FileHistorySize,
  type HistoryClient,
  type HistorySource,
  type ObjectVersions,
  type VersionRecord,
} from "./files/history-source";
export * from "./files/versions";
export * from "./files/details";
export { bindFilePanel, panelSource, type FilePanelRouteOptions } from "./app/file-panel-route";
export type { FileLayout } from "./components/file-list";
export type { WriteOperation } from "./components/file-browser-writes";
export * from "./files/model";
export {
  ClientFileSource,
  type FileClient,
  type FileSource,
  type RestoreOptions,
  type SourceOptions,
} from "./files/source";
export { MemoryFileSource, type MemoryFile, type MemoryOptions } from "./files/memory-source";
export { MediaHandleController, MAX_REMINTS, type MediaState } from "./files/media-handle";
export { columnsFor, revealScrollTop, visibleWindow } from "./files/window";
export { listKeyTarget } from "./files/keys";
export { uploadName, uploadFiles, moveTarget, MAX_UPLOAD_FILES } from "./files/ops";
export {
  builtInRenderers,
  pickRenderer,
  type PreviewContext,
  type PreviewRenderer,
} from "./files/preview";
export { treeKeyAction, type KeyItem, type TreeKeyAction } from "./components/nav-tree-keys";
export {
  EMPTY_NAVIGATION_PREFERENCES,
  KIND_LABEL,
  MAX_COLLAPSED,
  MAX_DEPTH,
  MAX_HANDLE_BYTES,
  MAX_LIBRARIES,
  MAX_PAGES,
  MAX_PINNED,
  MAX_SECTIONS,
  NAVIGATION_KINDS,
  PINNED_SECTION,
  breadcrumbItems,
  isHandle,
  isNotFound,
  moveSection,
  navigationHref,
  nodeName,
  readStoredPreferences,
  sectionOrder,
  serializeNavigationPreferences,
  unavailableName,
  withMember,
  type BreadcrumbItem,
  type NavigationChange,
  type NavigationKind,
  type NavigationNode,
  type NavigationPage,
  type NavigationPreferences,
  type NavigationSource,
  type PinnedDestination,
  type StoredPreferences,
} from "./navigation/model";
export {
  readLevel,
  readNavigation,
  readPins,
  type NavigationData,
  type NavigationLevel,
  type ResolvedPin,
} from "./navigation/loader";
export {
  NAVIGATION_STORAGE_KEY,
  NavigationStore,
  createNavigationStore,
  sharedNavigationStore,
  type NavigationEnvironment,
} from "./navigation/store";
export { MemoryNavigationSource, type FixtureNode } from "./navigation/source";
export { ClientNavigationSource, type NavigationClient } from "./navigation/client-source";
export { FsDriveView, type DriveViewOptions, type ZipDownload } from "./components/fs-drive-view";
export {
  FsDestinationPicker,
  type DestinationChoice,
  type DestinationPickerOptions,
} from "./components/fs-destination-picker";
export {
  FsNewMenu,
  NEW_ACTION_EVENT,
  type NewAction,
  type NewMenuOptions,
} from "./components/fs-new-menu";
export { FILTER_EVENT } from "./components/fs-app-shell";
export { DriveList, type DriveItem, type DriveLayout } from "./components/drive-list";
export { Selection, MAX_SELECTED, type SelectionState } from "./drive/selection";
export {
  ZIP_MAX_BYTES,
  ZIP_MAX_FILES,
  driveRow,
  nextSort,
  relativeTime as driveRelativeTime,
  scopeText,
  zipCheck,
  type DriveRow,
  type ViewQuery,
} from "./drive/model";
export { ClientViewSource, type ViewSource } from "./drive/source";
export { singleOpenLibrary } from "./drive/library";
export { MemoryViewSource } from "./drive/memory-source";
export {
  DestinationUnavailableError,
  DriveActions,
  UnavailableActionError,
  canAct,
} from "./drive/actions";
export { DEFAULT_PREFERENCES, parsePreferences, type UserPreferences } from "./preferences/model";
export {
  ClientPreferencesSource,
  MemoryPreferencesSource,
  type PreferencesSource,
} from "./preferences/source";
export { PreferencesService, type PreferencesMode } from "./preferences/service";
export { bindDriveRoute, firstLibrary, type DriveRouteOptions } from "./app/drive-route";
export { bindStorage, readStorage, type StorageClient } from "./app/storage-route";
export {
  MAX_CHANGE_PARENTS,
  MAX_PAGE_NODES,
  parseNavigationChange,
  parseNavigationPage,
} from "./navigation/contract";
export { FsNavigationView, type NavigationViewOptions } from "./components/fs-navigation-view";
export { HOST_STATEMENT_STYLE, buildHostStatement } from "./components/host-statement";
export { FsSettings, type SettingsClient, type SettingsOptions } from "./components/fs-settings";
export { FsLibraryStart } from "./components/fs-library-start";
export { FsLibraryDetail } from "./components/fs-library-detail";
export { FsLibraryRepair, type RepairPageOptions } from "./components/fs-library-repair";
export type { LibraryPageOptions } from "./components/library-parts";
export { bindLibraryRoute } from "./app/library-route";
export * from "./libraries/model";
export { ClientLibrarySource, type LibraryClient } from "./libraries/client-source";
export {
  parseLibraryView,
  parseLocationChoice,
  parseRebuildReport,
  parseRepairAnswer,
  parseRepairCandidates,
  parseSnapshotCopyReport,
} from "./libraries/contract";
export { libraryFailure, type Failure, type LibraryAction } from "./libraries/errors";
export { LibraryRegistry } from "./libraries/registry";
export { deviceStorage, type DeviceStorage } from "./app/device-storage";
export type { MonitorClient, MonitorOptions, TokenStore } from "./components/fs-monitor";
export * from "./api/client";
export {
  commandEnvelope,
  commandOutcome,
  conflictOutcome,
  isCommitted,
  newOperationId,
  parseContentRange,
  rangeHeader,
  type ByteRange,
  type CommandEnvelope,
  type CommandOutcome,
  type CommandOutcomeKind,
  type ContentRange,
  type EnvelopeInput,
  type PortError,
} from "./api/commands";
export { createHostClient, DESKTOP_EVENT, LAUNCH_TOKEN_GLOBAL, launchToken } from "./api/host";
export {
  ALL_FILES_ROUTE,
  LOGS_ROUTE,
  MONITOR_ROUTE,
  NAVIGATION_ROUTE_PREFIX,
  RECENT_ROUTE,
  RECENTLY_DELETED_ROUTE,
  SETTINGS_ROUTE,
  bindFilesRoute,
  bindMonitorRoute,
  bindNavigationRoute,
  bindSettingsRoute,
  memoryTokenStore,
  parseNavigationHash,
  type FilePanelSources,
  type FilesRouteOptions,
  type NavigationRouteOptions,
  type RouteHost,
} from "./app/routes";
export {
  accessibilityViolations,
  accessibleName,
  type A11yRule,
  type A11yViolation,
} from "./a11y/audit";
export { deepActiveElement, focusElement, recoverFocus, replaceKeepingFocus } from "./a11y/focus";
export {
  DEFAULT_LOCALE,
  applyLocale,
  resolveLocale,
  type Locale,
  type TextDirection,
} from "./a11y/locale";
export { REDUCED_MOTION_QUERY, prefersReducedMotion, type MotionEnvironment } from "./a11y/motion";
export { A11Y_STYLE } from "./components/a11y-style";
export { denialLabel, reasonLabel, type DenialText } from "./components/denial";
export { logoSource, type Distribution, type DistributionLogo } from "./distribution";
export * from "./theme/index";
export {
  DARK_SCHEME_QUERY,
  ThemeStore,
  createThemeStore,
  type ThemeEnvironment,
  type ThemeMedia,
  type ThemeStorage,
} from "./theme/theme-store";

/**
 * Register every FileSquid custom element once. Safe to call repeatedly. Elements that other
 * elements create (the theme control, the states element) are defined first.
 */
export function defineElements(): void {
  if (!customElements.get("fs-theme-control")) {
    customElements.define("fs-theme-control", FsThemeControl);
  }
  if (!customElements.get("fs-states")) customElements.define("fs-states", FsStates);
  if (!customElements.get("fs-nav-tree")) customElements.define("fs-nav-tree", FsNavTree);
  if (!customElements.get("fs-breadcrumb")) customElements.define("fs-breadcrumb", FsBreadcrumb);
  if (!customElements.get("fs-navigation-view")) {
    customElements.define("fs-navigation-view", FsNavigationView);
  }
  if (!customElements.get("fs-file-preview")) {
    customElements.define("fs-file-preview", FsFilePreview);
  }
  if (!customElements.get("fs-file-browser")) {
    customElements.define("fs-file-browser", FsFileBrowser);
  }
  if (!customElements.get("fs-destination-picker")) {
    customElements.define("fs-destination-picker", FsDestinationPicker);
  }
  if (!customElements.get("fs-drive-view")) customElements.define("fs-drive-view", FsDriveView);
  if (!customElements.get("fs-new-menu")) customElements.define("fs-new-menu", FsNewMenu);
  if (!customElements.get("fs-details-panel")) {
    customElements.define("fs-details-panel", FsDetailsPanel);
  }
  if (!customElements.get("fs-metadata-panel")) {
    customElements.define("fs-metadata-panel", FsMetadataPanel);
  }
  if (!customElements.get("fs-versions-panel")) {
    customElements.define("fs-versions-panel", FsVersionsPanel);
  }
  if (!customElements.get("fs-file-panel")) customElements.define("fs-file-panel", FsFilePanel);
  if (!customElements.get("fs-host-indicator")) {
    customElements.define("fs-host-indicator", FsHostIndicator);
  }
  if (!customElements.get("fs-settings")) customElements.define("fs-settings", FsSettings);
  if (!customElements.get("fs-library-start")) {
    customElements.define("fs-library-start", FsLibraryStart);
  }
  if (!customElements.get("fs-library-detail")) {
    customElements.define("fs-library-detail", FsLibraryDetail);
  }
  if (!customElements.get("fs-library-repair")) {
    customElements.define("fs-library-repair", FsLibraryRepair);
  }
  if (!customElements.get("fs-app-shell")) customElements.define("fs-app-shell", FsAppShell);
  if (!customElements.get("fs-ocfl-findings")) {
    customElements.define("fs-ocfl-findings", FsOcflFindings);
  }
  if (!customElements.get("fs-monitor")) customElements.define("fs-monitor", FsMonitor);
}
