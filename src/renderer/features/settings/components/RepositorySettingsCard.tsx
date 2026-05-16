import React from 'react';
import { useAppSettingsKey } from '@renderer/features/settings/use-app-settings-key';
import { Switch } from '@renderer/lib/ui/switch';
import { ResetToDefaultButton } from './ResetToDefaultButton';
import { SettingRow } from './SettingRow';

const RepositorySettingsCard: React.FC = () => {
  const {
    value: project,
    update: updateProject,
    isLoading: projectLoading,
    isSaving: projectSaving,
    isFieldOverridden: isProjectFieldOverridden,
    resetField: resetProjectField,
  } = useAppSettingsKey('project');
  const {
    value: localProject,
    update: updateLocalProject,
    isLoading: localProjectLoading,
    isSaving: localProjectSaving,
    isFieldOverridden: isLocalProjectFieldOverridden,
    resetField: resetLocalProjectField,
  } = useAppSettingsKey('localProject');

  const pushOnCreate = project?.pushOnCreate ?? true;
  const writeAgentConfigToGitIgnore = localProject?.writeAgentConfigToGitIgnore ?? true;
  const projectBusy = projectLoading || projectSaving;
  const localProjectBusy = localProjectLoading || localProjectSaving;

  return (
    <div className="grid gap-8">
      <SettingRow
        title="Auto-push on create"
        description="Push the new branch to the selected project remote and set upstream after creation."
        control={
          <>
            <ResetToDefaultButton
              visible={isProjectFieldOverridden('pushOnCreate')}
              defaultLabel="on"
              onReset={() => resetProjectField('pushOnCreate')}
              disabled={projectBusy}
            />
            <Switch
              checked={pushOnCreate}
              onCheckedChange={(checked) => updateProject({ pushOnCreate: checked })}
              disabled={projectBusy}
              aria-label="Enable automatic push on create"
            />
          </>
        }
      />
      <SettingRow
        title="Auto-update .gitignore"
        description="When Emdash writes CLI hook configs, also add their paths to .gitignore."
        control={
          <>
            <ResetToDefaultButton
              visible={isLocalProjectFieldOverridden('writeAgentConfigToGitIgnore')}
              defaultLabel="on"
              onReset={() => resetLocalProjectField('writeAgentConfigToGitIgnore')}
              disabled={localProjectBusy}
            />
            <Switch
              checked={writeAgentConfigToGitIgnore}
              onCheckedChange={(checked) =>
                updateLocalProject({ writeAgentConfigToGitIgnore: checked })
              }
              disabled={localProjectBusy}
              aria-label="Enable .gitignore updates for CLI hook configs"
            />
          </>
        }
      />
    </div>
  );
};

export default RepositorySettingsCard;
