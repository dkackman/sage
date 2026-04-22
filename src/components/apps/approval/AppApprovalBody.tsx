import type { SageAppCapabilityDefinitionView } from '@/bindings';
import type { PendingApproval } from '@/components/apps/AppApprovalStrip.tsx';
import { SendXchApprovalCard } from '@/components/apps/approval/SendXchApprovalCard.tsx';
import { CapabilityGrantApprovalCard } from '@/components/apps/approval/CapabilityGrantApprovalCard.tsx';
import { NetworkWhitelistGrantApprovalCard } from '@/components/apps/approval/NetworkWhitelistGrantApprovalCard.tsx';

interface Props {
  approval: Exclude<PendingApproval, null>;
  expanded: boolean;
  capabilityRegistry: Record<string, SageAppCapabilityDefinitionView>;
}

export function AppApprovalBody({
  approval,
  expanded,
  capabilityRegistry,
}: Props) {
  switch (approval.kind) {
    case 'send_xch':
      return <SendXchApprovalCard approval={approval} expanded={expanded} />;
    case 'capability_grant':
      return (
        <CapabilityGrantApprovalCard
          approval={approval}
          expanded={expanded}
          capabilityRegistry={capabilityRegistry}
        />
      );
    case 'network_whitelist_grant':
      return (
        <NetworkWhitelistGrantApprovalCard
          approval={approval}
          expanded={expanded}
        />
      );
  }
}
