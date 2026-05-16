import { observer } from 'mobx-react-lite';
import { Activity } from 'react';
import { useWorkspaceViewModel } from '@renderer/features/workspaces/workspace-view-context';
import { SidebarConversationsList } from '../conversations/sidebar-conversations-list';
import { ChangesPanel } from '../diff-view/changes-panel/changes-panel';
import { EditorFileTree } from '../editor/editor-file-tree';

export const WorkspaceSidebar = observer(function WorkspaceSidebar() {
  const workspaceView = useWorkspaceViewModel();
  const { isSidebarCollapsed, sidebarTab: activeTab } = workspaceView;
  return (
    <Activity mode={isSidebarCollapsed ? 'hidden' : 'visible'}>
      <div className="min-h-0 h-full overflow-hidden">
        <Activity mode={activeTab === 'conversations' ? 'visible' : 'hidden'}>
          <SidebarConversationsList />
        </Activity>
        <Activity mode={activeTab === 'changes' ? 'visible' : 'hidden'}>
          <ChangesPanel />
        </Activity>
        <Activity mode={activeTab === 'files' ? 'visible' : 'hidden'}>
          <EditorFileTree />
        </Activity>
      </div>
    </Activity>
  );
});
