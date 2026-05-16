// Renderer-facing types for the application settings. Previously these
// were inferred from Zod schemas in `@main/core/settings/schema`. With
// the Electron tree removed, the renderer no longer has access to
// those schemas — declared directly here as TypeScript types so the
// renderer can build without depending on `@main`.
//
// The Tauri host validates incoming settings JSON on its own when the
// `app_settings_*` commands land; until then the renderer treats these
// as best-effort shapes.

import type { OpenInAppId } from './openInApps';
import type { APP_SHORTCUTS } from './shortcuts';

export interface LocalProjectSettings {
  defaultProjectsDirectory: string;
  defaultWorktreeDirectory: string;
  writeAgentConfigToGitIgnore: boolean;
}

export interface ProjectSettings {
  pushOnCreate: boolean;
  branchPrefix: string;
  tmuxByDefault: boolean;
}

export interface NotificationSettings {
  enabled: boolean;
  sound: boolean;
  osNotifications: boolean;
  soundFocusMode: 'always' | 'unfocused';
}

export interface TaskSettings {
  autoGenerateName: boolean;
  autoTrustWorktrees: boolean;
  createBranchAndWorktree: boolean;
}

export type AgentAutoApproveDefaults = Partial<Record<string, boolean>>;

export interface TerminalSettings {
  fontFamily?: string;
  fontSize?: number;
  autoCopyOnSelection: boolean;
}

export type Theme = 'emlight' | 'emdark' | null | undefined;

export interface InterfaceSettings {
  taskHoverAction: 'delete' | 'archive';
  autoRightSidebarBehavior: boolean;
}

export interface ProviderCustomConfig {
  cli?: string;
  resumeFlag?: string;
  defaultArgs?: string[];
  autoApproveFlag?: string;
  initialPromptFlag?: string;
  sessionIdFlag?: string;
  sessionIdOnResumeOnly?: boolean;
  extraArgs?: string;
  env?: Record<string, string>;
}

export type ProviderCustomConfigs = Record<string, ProviderCustomConfig>;

export interface OpenInSettings {
  default: OpenInAppId;
  hidden: OpenInAppId[];
}

export interface BrowserPreviewSettings {
  enabled: boolean;
}

export interface ResourceMonitorSettings {
  enabled: boolean;
}

export type KeyboardSettings = Partial<Record<keyof typeof APP_SHORTCUTS, string | null>>;

export interface AppSettings {
  localProject: LocalProjectSettings;
  project: ProjectSettings;
  tasks: TaskSettings;
  agentAutoApproveDefaults: AgentAutoApproveDefaults;
  defaultAgent: string;
  reviewPrompt: string;
  keyboard: KeyboardSettings;
  notifications: NotificationSettings;
  theme: Theme;
  openIn: OpenInSettings;
  interface: InterfaceSettings;
  terminal: TerminalSettings;
  browserPreview: BrowserPreviewSettings;
  resourceMonitor: ResourceMonitorSettings;
}

export type AppSettingsKey = keyof AppSettings;

export const AppSettingsKeys: AppSettingsKey[] = [
  'localProject',
  'project',
  'tasks',
  'agentAutoApproveDefaults',
  'defaultAgent',
  'reviewPrompt',
  'keyboard',
  'notifications',
  'theme',
  'openIn',
  'interface',
  'terminal',
  'browserPreview',
  'resourceMonitor',
];
