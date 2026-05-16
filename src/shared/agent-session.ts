import type { AgentProviderId } from '@shared/agent-provider-registry';

export interface AgentSessionConfig {
  workspaceId: string;
  conversationId: string;
  providerId: AgentProviderId;
  command: string;
  args: string[];
  cwd: string;
  sessionId?: string;
  shellSetup?: string;
  tmuxSessionName?: string;
  autoApprove: boolean;
  resume: boolean;
}
