import { LogIn, LogOut, User } from 'lucide-react';
import githubSvg from '@/assets/images/Github.svg?raw';
import { useTheme } from '@renderer/lib/hooks/useTheme';
import { useShowModal } from '@renderer/lib/modal/modal-provider';
import { useGithubContext } from '@renderer/lib/providers/github-context-provider';
import { Button } from '@renderer/lib/ui/button';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@renderer/lib/ui/tooltip';

const tokenSourceLabel = (source: 'secure_storage' | 'cli' | null): string => {
  if (source === 'cli') return 'Signed in via gh CLI';
  if (source === 'secure_storage') return 'Signed in via device flow';
  return 'Signed in';
};

function GithubBadge() {
  const { effectiveTheme } = useTheme();
  const isDark = effectiveTheme === 'emdark';
  const processed = isDark
    ? githubSvg
        .replace(/\bfill="[^"]*"/g, 'fill="currentColor"')
        .replace(/\bstroke="[^"]*"/g, 'stroke="currentColor"')
    : githubSvg;
  return (
    <span
      className={`absolute -right-1 -bottom-1 inline-flex h-[18px] w-[18px] items-center justify-center rounded-full bg-background ring-2 ring-background [&_svg]:h-3 [&_svg]:w-3 ${
        isDark ? 'text-primary' : ''
      }`}
      dangerouslySetInnerHTML={{ __html: processed }}
    />
  );
}

export function AccountTab() {
  const {
    authenticated,
    user,
    tokenSource,
    isInitialized,
    isLoading,
    githubLoading,
    handleGithubConnect,
    cancelGithubConnect,
    logout,
  } = useGithubContext();
  const showConfirmDisconnect = useShowModal('confirmActionModal');

  const isCliManaged = authenticated && tokenSource === 'cli';
  const busy = isLoading || githubLoading;

  if (!isInitialized) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        Loading account…
      </div>
    );
  }

  if (!authenticated || !user) {
    return (
      <div className="rounded-xl border border-border/60 bg-muted/10 p-5">
        <div className="flex items-start gap-4">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full border border-border/60 bg-muted">
            <User className="h-6 w-6 text-muted-foreground" />
          </div>
          <div className="flex-1 space-y-2">
            <div>
              <p className="text-sm font-medium text-foreground">No GitHub account connected</p>
              <p className="text-xs text-muted-foreground">
                Connect GitHub so Emdash can read repositories and act on your behalf.
              </p>
            </div>
            <Button
              type="button"
              size="sm"
              onClick={busy ? cancelGithubConnect : handleGithubConnect}
            >
              <LogIn className="mr-1.5 h-3.5 w-3.5" />
              {busy ? 'Connecting…' : 'Connect GitHub'}
            </Button>
          </div>
        </div>
      </div>
    );
  }

  const displayName = user.name?.trim() || user.login;

  const confirmDisconnect = () => {
    showConfirmDisconnect({
      title: 'Disconnect GitHub',
      description: 'This will sign you out of GitHub in Emdash.',
      confirmLabel: 'Disconnect',
      onSuccess: () => {
        void logout();
      },
    });
  };

  return (
    <div className="rounded-xl border border-border/60 bg-muted/10 p-5">
      <div className="flex items-center gap-4">
        <div className="relative shrink-0">
          {user.avatar_url ? (
            <img
              src={user.avatar_url}
              alt={user.login}
              className="h-12 w-12 rounded-full border border-border/60 object-cover"
            />
          ) : (
            <div className="flex h-12 w-12 items-center justify-center rounded-full border border-border/60 bg-muted">
              <User className="h-6 w-6 text-muted-foreground" />
            </div>
          )}
          <GithubBadge />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-foreground">{displayName}</span>
            <span className="truncate text-xs text-muted-foreground">@{user.login}</span>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5">
            {user.email && (
              <span className="min-w-0 truncate text-xs text-muted-foreground">{user.email}</span>
            )}
            <span className="shrink-0 text-[11px] text-muted-foreground/80">
              {tokenSourceLabel(tokenSource)}
            </span>
          </div>
        </div>
        {isCliManaged ? (
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger
                className="inline-flex h-8 shrink-0 cursor-default items-center gap-1.5 rounded-md border border-input bg-background px-3 text-xs font-medium opacity-70"
                aria-label="Disconnect via gh CLI"
              >
                <LogOut className="h-3.5 w-3.5" />
                Disconnect
              </TooltipTrigger>
              <TooltipContent side="top">
                <p className="text-xs">Run `gh auth logout` in your terminal to disconnect</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        ) : (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={confirmDisconnect}
            disabled={isLoading}
          >
            <LogOut className="mr-1.5 h-3.5 w-3.5" />
            Disconnect
          </Button>
        )}
      </div>
    </div>
  );
}
