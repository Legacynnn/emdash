export interface GeneralSession {
  type: 'general';
  config: GeneralSessionConfig;
}

export interface GeneralSessionConfig {
  workspaceId?: string;
  cwd: string;
  projectPath?: string;
  shellSetup?: string;
  tmuxSessionName?: string;
  command?: string;
  args?: string[];
}
