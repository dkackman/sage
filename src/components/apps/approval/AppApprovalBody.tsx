import { SendXchApprovalCard } from '@/components/apps/approval/SendXchApprovalCard.tsx';
import { CapabilityGrantApprovalCard } from '@/components/apps/approval/CapabilityGrantApprovalCard.tsx';
import { NetworkWhitelistGrantApprovalCard } from '@/components/apps/approval/NetworkWhitelistGrantApprovalCard.tsx';
import type { RustBridgeApprovalEvent } from '@/bindings';

interface Props {
  approval: RustBridgeApprovalEvent;
  expanded: boolean;
}

export function AppApprovalBody({ approval, expanded }: Props) {
  const req = approval.approval;

  switch (req.kind) {
    case 'sendXch':
      return <SendXchApprovalCard approval={req} expanded={expanded} />;

    case 'capabilityGrant':
      return <CapabilityGrantApprovalCard approval={req} expanded={expanded} />;

    case 'networkWhitelistGrant':
      return (
        <NetworkWhitelistGrantApprovalCard approval={req} expanded={expanded} />
      );
  }
}
