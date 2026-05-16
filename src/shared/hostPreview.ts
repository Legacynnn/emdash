export type HostPreviewEvent = {
  type: 'url' | 'setup' | 'exit';
  workspaceId: string;
  terminalId?: string;
  url?: string;
  status?: 'starting' | 'line' | 'done' | 'error';
  line?: string;
};
