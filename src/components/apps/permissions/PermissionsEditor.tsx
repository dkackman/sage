import { useMemo, useState } from 'react';
import type {
  InstalledSageApp,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
} from '@/bindings';
import { Checkbox } from '@/components/ui/checkbox';
import { Button } from '@/components/ui/button';
import {
  ChevronDown,
  ChevronRight,
  Globe,
  HardDrive,
  KeyRound,
  Radio,
  Shield,
} from 'lucide-react';

interface Props {
  app: InstalledSageApp;
  grantedPermissions: SageGrantedPermissions;
  onGrantedPermissionsChange?: (next: SageGrantedPermissions) => void;
  editable?: boolean;
}

type PermissionKind = 'capability' | 'network';

interface PermissionEntry {
  id: string;
  kind: PermissionKind;
  key: string;
  label: string;
  detail?: string;
  required: boolean;
  granted: boolean;
  sensitivityRank: number;
}

interface PermissionGroupNode {
  id: string;
  label: string;
  children: PermissionGroupNode[];
  entries: PermissionEntry[];
  sensitivityRank: number;
}

function networkKey(entry: SageNetworkPermissionTarget): string {
  return `${entry.scheme}://${entry.host}`;
}

function sortNetworkEntries(
  entries: SageNetworkPermissionTarget[],
): SageNetworkPermissionTarget[] {
  return [...entries].sort((a, b) => {
    const aKey = networkKey(a);
    const bKey = networkKey(b);
    return aKey.localeCompare(bKey);
  });
}

function titleCasePart(value: string): string {
  if (!value) {
    return value;
  }

  return value.charAt(0).toUpperCase() + value.slice(1);
}

function segmentLabel(segment: string): string {
  return segment.split('_').filter(Boolean).map(titleCasePart).join(' ');
}

function formatCapabilityLeafLabel(key: string): string {
  const parts = key.split('.');
  return segmentLabel(parts[parts.length - 1] ?? key);
}

function normalizeKey(key: string): string {
  return key.trim().toLowerCase();
}

function isSecretCapability(key: string): boolean {
  const normalized = normalizeKey(key);

  return (
    normalized.includes('secret') ||
    normalized.includes('mnemonic') ||
    normalized.includes('seed') ||
    normalized.includes('private_key') ||
    normalized.includes('privatekey') ||
    normalized.includes('keychain') ||
    normalized.includes('wallet.sign') ||
    normalized.includes('sign_spend') ||
    normalized.includes('export_key')
  );
}

function isPersistentStorageCapability(key: string): boolean {
  const normalized = normalizeKey(key);

  return (
    normalized === 'persistent_storage' ||
    normalized.includes('persistent_storage') ||
    normalized.includes('persistentstorage')
  );
}

function isExternallyObservableCapability(key: string): boolean {
  const normalized = normalizeKey(key);

  return (
    normalized.includes('send') ||
    normalized.includes('transfer') ||
    normalized.includes('submit') ||
    normalized.includes('broadcast') ||
    normalized.includes('publish') ||
    normalized.includes('issue') ||
    normalized.includes('mint') ||
    normalized.includes('offer') ||
    normalized.includes('exercise') ||
    normalized.includes('message') ||
    normalized.includes('sign_message')
  );
}

function capabilitySensitivityRank(key: string): number {
  if (isSecretCapability(key)) {
    return 0;
  }

  if (isPersistentStorageCapability(key)) {
    return 2;
  }

  if (isExternallyObservableCapability(key)) {
    return 3;
  }

  return 4;
}

function buildCapabilityEntries(
  requestedRequired: string[],
  requestedOptional: string[],
  grantedCapabilities: string[],
): PermissionEntry[] {
  const grantedSet = new Set(grantedCapabilities);

  const requiredEntries: PermissionEntry[] = requestedRequired.map((key) => ({
    id: `capability:${key}`,
    kind: 'capability',
    key,
    label: formatCapabilityLeafLabel(key),
    detail: key,
    required: true,
    granted: true,
    sensitivityRank: capabilitySensitivityRank(key),
  }));

  const optionalEntries: PermissionEntry[] = requestedOptional.map((key) => ({
    id: `capability:${key}`,
    kind: 'capability',
    key,
    label: formatCapabilityLeafLabel(key),
    detail: key,
    required: false,
    granted: grantedSet.has(key),
    sensitivityRank: capabilitySensitivityRank(key),
  }));

  return [...requiredEntries, ...optionalEntries];
}

function buildNetworkEntries(
  requestedRequired: SageNetworkPermissionTarget[],
  requestedOptional: SageNetworkPermissionTarget[],
  grantedNetworkWhitelist: SageNetworkPermissionTarget[],
): PermissionEntry[] {
  const grantedSet = new Set(
    grantedNetworkWhitelist.map((entry) => networkKey(entry)),
  );

  const requiredEntries: PermissionEntry[] = requestedRequired.map((entry) => {
    const key = networkKey(entry);

    return {
      id: `network:${key}`,
      kind: 'network',
      key,
      label: key,
      detail: undefined,
      required: true,
      granted: true,
      sensitivityRank: 1,
    };
  });

  const optionalEntries: PermissionEntry[] = requestedOptional.map((entry) => {
    const key = networkKey(entry);

    return {
      id: `network:${key}`,
      kind: 'network',
      key,
      label: key,
      detail: undefined,
      required: false,
      granted: grantedSet.has(key),
      sensitivityRank: 1,
    };
  });

  return [...requiredEntries, ...optionalEntries];
}

function sortPermissionEntries(entries: PermissionEntry[]): PermissionEntry[] {
  return [...entries].sort((a, b) => {
    if (a.sensitivityRank !== b.sensitivityRank) {
      return a.sensitivityRank - b.sensitivityRank;
    }

    if (a.kind !== b.kind) {
      return a.kind.localeCompare(b.kind);
    }

    return a.key.localeCompare(b.key);
  });
}

function entryBadge(entry: PermissionEntry): string | null {
  if (entry.kind === 'network') {
    return 'Network';
  }

  if (isSecretCapability(entry.key)) {
    return 'Secrets';
  }

  if (isPersistentStorageCapability(entry.key)) {
    return 'Storage';
  }

  if (isExternallyObservableCapability(entry.key)) {
    return 'External';
  }

  return null;
}

function groupIcon(node: PermissionGroupNode) {
  const normalized = normalizeKey(node.id);

  if (normalized === 'network') {
    return <Globe className='h-4 w-4' />;
  }

  if (normalized === 'persistent_storage') {
    return <HardDrive className='h-4 w-4' />;
  }

  if (normalized.includes('secret') || normalized.includes('wallet')) {
    return <KeyRound className='h-4 w-4' />;
  }

  if (normalized.includes('send') || normalized.includes('submit')) {
    return <Radio className='h-4 w-4' />;
  }

  return <Shield className='h-4 w-4' />;
}

function makeNode(id: string, label: string): PermissionGroupNode {
  return {
    id,
    label,
    children: [],
    entries: [],
    sensitivityRank: 999,
  };
}

function updateNodeSensitivity(
  node: PermissionGroupNode,
  rank: number,
): PermissionGroupNode {
  node.sensitivityRank = Math.min(node.sensitivityRank, rank);
  return node;
}

function buildGroupedPermissionTree(
  entries: PermissionEntry[],
): PermissionGroupNode[] {
  const roots: PermissionGroupNode[] = [];

  const networkEntries = entries.filter((entry) => entry.kind === 'network');
  if (networkEntries.length > 0) {
    const networkNode = makeNode('network', 'Network access');
    networkNode.entries = sortPermissionEntries(networkEntries);
    networkNode.sensitivityRank = 1;
    roots.push(networkNode);
  }

  const persistentEntries = entries.filter(
    (entry) =>
      entry.kind === 'capability' && isPersistentStorageCapability(entry.key),
  );
  if (persistentEntries.length > 0) {
    const persistentNode = makeNode('persistent_storage', 'Persistent storage');
    persistentNode.entries = sortPermissionEntries(persistentEntries);
    persistentNode.sensitivityRank = 2;
    roots.push(persistentNode);
  }

  const generalCapabilityEntries = entries.filter(
    (entry) =>
      entry.kind === 'capability' && !isPersistentStorageCapability(entry.key),
  );

  const capabilityRoot = makeNode('capabilities_root', 'Capabilities');

  for (const entry of generalCapabilityEntries) {
    const parts = entry.key.split('.');
    const leafParentParts = parts.slice(0, -1);

    if (leafParentParts.length === 0) {
      capabilityRoot.entries.push(entry);
      updateNodeSensitivity(capabilityRoot, entry.sensitivityRank);
      continue;
    }

    let current = capabilityRoot;
    updateNodeSensitivity(current, entry.sensitivityRank);

    for (let index = 0; index < leafParentParts.length; index += 1) {
      const segment = leafParentParts[index];
      const fullPath = leafParentParts.slice(0, index + 1).join('.');
      let child = current.children.find((node) => node.id === fullPath);

      if (!child) {
        child = makeNode(fullPath, segmentLabel(segment));
        current.children.push(child);
      }

      updateNodeSensitivity(child, entry.sensitivityRank);
      current = child;
    }

    current.entries.push(entry);
    updateNodeSensitivity(current, entry.sensitivityRank);
  }

  function sortNode(node: PermissionGroupNode) {
    node.entries = sortPermissionEntries(node.entries);

    node.children.sort((a, b) => {
      if (a.sensitivityRank !== b.sensitivityRank) {
        return a.sensitivityRank - b.sensitivityRank;
      }

      return a.label.localeCompare(b.label);
    });

    for (const child of node.children) {
      sortNode(child);
    }
  }

  sortNode(capabilityRoot);

  if (capabilityRoot.entries.length > 0 || capabilityRoot.children.length > 0) {
    roots.push(...capabilityRoot.children);

    if (capabilityRoot.entries.length > 0) {
      const miscNode = makeNode('misc', 'Other capabilities');
      miscNode.entries = capabilityRoot.entries;
      miscNode.sensitivityRank = capabilityRoot.sensitivityRank;
      roots.push(miscNode);
    }
  }

  roots.sort((a, b) => {
    if (a.sensitivityRank !== b.sensitivityRank) {
      return a.sensitivityRank - b.sensitivityRank;
    }

    return a.label.localeCompare(b.label);
  });

  return roots;
}

function PermissionRow({
  entry,
  editable,
  onToggle,
}: {
  entry: PermissionEntry;
  editable: boolean;
  onToggle: (entry: PermissionEntry, nextGranted: boolean) => void;
}) {
  const badge = entryBadge(entry);

  return (
    <label className='flex items-start gap-3 rounded-xl border px-3 py-3 text-sm'>
      <Checkbox
        checked={entry.granted}
        disabled={!editable || entry.required}
        onCheckedChange={(checked) => {
          onToggle(entry, Boolean(checked));
        }}
        className='mt-0.5'
      />

      <div className='min-w-0 flex-1'>
        <div className='flex flex-wrap items-center gap-2'>
          <div
            className={
              entry.kind === 'network'
                ? 'min-w-0 truncate font-mono text-sm'
                : 'min-w-0 truncate font-medium'
            }
          >
            {entry.label}
          </div>

          {badge ? (
            <span className='rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground'>
              {badge}
            </span>
          ) : null}

          {entry.required ? (
            <span className='rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground'>
              Required
            </span>
          ) : null}
        </div>

        {entry.detail && entry.detail !== entry.label ? (
          <div className='mt-1 break-all text-xs text-muted-foreground'>
            {entry.detail}
          </div>
        ) : null}
      </div>
    </label>
  );
}

function PermissionGroupBlock({
  node,
  editable,
  onToggleEntry,
}: {
  node: PermissionGroupNode;
  editable: boolean;
  onToggleEntry: (entry: PermissionEntry, nextGranted: boolean) => void;
}) {
  return (
    <div className='space-y-3 rounded-2xl border p-4'>
      <div className='flex items-center gap-2'>
        <div className='text-muted-foreground'>{groupIcon(node)}</div>
        <div className='text-sm font-medium'>{node.label}</div>
      </div>

      {node.entries.length > 0 ? (
        <div className='space-y-2'>
          {node.entries.map((entry) => (
            <PermissionRow
              key={entry.id}
              entry={entry}
              editable={editable}
              onToggle={onToggleEntry}
            />
          ))}
        </div>
      ) : null}

      {node.children.length > 0 ? (
        <div className='space-y-3'>
          {node.children.map((child) => (
            <PermissionGroupBlock
              key={child.id}
              node={child}
              editable={editable}
              onToggleEntry={onToggleEntry}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function PermissionSection({
  title,
  subtitle,
  groups,
  editable,
  collapsed,
  onToggleCollapsed,
  onToggleEntry,
}: {
  title: string;
  subtitle?: string;
  groups: PermissionGroupNode[];
  editable: boolean;
  collapsed?: boolean;
  onToggleCollapsed?: () => void;
  onToggleEntry: (entry: PermissionEntry, nextGranted: boolean) => void;
}) {
  if (groups.length === 0) {
    return null;
  }

  const itemCount = groups.reduce((count, group) => {
    function countNode(node: PermissionGroupNode): number {
      return (
        node.entries.length +
        node.children.reduce((sum, child) => sum + countNode(child), 0)
      );
    }

    return count + countNode(group);
  }, 0);

  const contentHidden = Boolean(collapsed);

  return (
    <div className='space-y-3'>
      <div className='flex items-center justify-between gap-3'>
        <div>
          <h3 className='text-sm font-medium'>{title}</h3>
          {subtitle ? (
            <div className='mt-1 text-xs text-muted-foreground'>{subtitle}</div>
          ) : null}
        </div>

        {onToggleCollapsed ? (
          <Button
            type='button'
            variant='ghost'
            size='sm'
            className='h-8 gap-1 px-2 text-muted-foreground'
            onClick={onToggleCollapsed}
          >
            {contentHidden ? (
              <ChevronRight className='h-4 w-4' />
            ) : (
              <ChevronDown className='h-4 w-4' />
            )}
            {itemCount}
          </Button>
        ) : null}
      </div>

      {!contentHidden ? (
        <div className='space-y-3'>
          {groups.map((group) => (
            <PermissionGroupBlock
              key={group.id}
              node={group}
              editable={editable}
              onToggleEntry={onToggleEntry}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function PermissionsEditor({
  app,
  grantedPermissions,
  onGrantedPermissionsChange,
  editable = true,
}: Props) {
  const manifest = app.pendingUpdate?.manifest ?? app.activeSnapshot.manifest;
  const [showOptional, setShowOptional] = useState(false);

  const grantedCapabilities = grantedPermissions.capabilities ?? [];
  const grantedNetworkWhitelist = grantedPermissions.network.whitelist ?? [];

  const requestedRequiredCapabilities =
    manifest.permissions?.capabilities?.required ?? [];
  const requestedOptionalCapabilities =
    manifest.permissions?.capabilities?.optional ?? [];

  const requestedRequiredNetwork =
    manifest.permissions?.network?.whitelist?.required ?? [];
  const requestedOptionalNetwork =
    manifest.permissions?.network?.whitelist?.optional ?? [];

  const requiredEntries = useMemo(() => {
    const capabilityEntries = buildCapabilityEntries(
      requestedRequiredCapabilities,
      [],
      grantedCapabilities,
    );

    const networkEntries = buildNetworkEntries(
      requestedRequiredNetwork,
      [],
      grantedNetworkWhitelist,
    );

    return sortPermissionEntries([...capabilityEntries, ...networkEntries]);
  }, [
    requestedRequiredCapabilities,
    grantedCapabilities,
    requestedRequiredNetwork,
    grantedNetworkWhitelist,
  ]);

  const optionalEntries = useMemo(() => {
    const capabilityEntries = buildCapabilityEntries(
      [],
      requestedOptionalCapabilities,
      grantedCapabilities,
    );

    const networkEntries = buildNetworkEntries(
      [],
      requestedOptionalNetwork,
      grantedNetworkWhitelist,
    );

    return sortPermissionEntries([...capabilityEntries, ...networkEntries]);
  }, [
    requestedOptionalCapabilities,
    grantedCapabilities,
    requestedOptionalNetwork,
    grantedNetworkWhitelist,
  ]);

  const requiredGroups = useMemo(
    () => buildGroupedPermissionTree(requiredEntries),
    [requiredEntries],
  );

  const optionalGroups = useMemo(
    () => buildGroupedPermissionTree(optionalEntries),
    [optionalEntries],
  );

  function emitGrantedPermissions(next: SageGrantedPermissions) {
    onGrantedPermissionsChange?.(next);
  }

  function handleToggleEntry(entry: PermissionEntry, nextGranted: boolean) {
    if (!editable || entry.required) {
      return;
    }

    if (entry.kind === 'capability') {
      const requiredSet = new Set(requestedRequiredCapabilities);
      const nextSet = new Set(grantedCapabilities);

      if (nextGranted) {
        nextSet.add(entry.key);
      } else {
        nextSet.delete(entry.key);
      }

      for (const requiredKey of requiredSet) {
        nextSet.add(requiredKey);
      }

      emitGrantedPermissions({
        ...grantedPermissions,
        capabilities: [...nextSet].sort((a, b) => a.localeCompare(b)),
      });

      return;
    }

    const requiredKeys = new Set(
      requestedRequiredNetwork.map((item) => networkKey(item)),
    );

    const nextOptional = requestedOptionalNetwork.filter((item) => {
      const key = networkKey(item);

      if (requiredKeys.has(key)) {
        return false;
      }

      if (key !== entry.key) {
        return grantedNetworkWhitelist.some(
          (grantedEntry) => networkKey(grantedEntry) === key,
        );
      }

      return nextGranted;
    });

    emitGrantedPermissions({
      ...grantedPermissions,
      network: {
        whitelist: sortNetworkEntries([
          ...requestedRequiredNetwork,
          ...nextOptional,
        ]),
      },
    });
  }

  if (requiredEntries.length === 0 && optionalEntries.length === 0) {
    return (
      <div className='rounded-xl border px-3 py-4 text-sm text-muted-foreground'>
        This app does not request any permissions.
      </div>
    );
  }

  return (
    <div className='space-y-5'>
      {requiredGroups.length > 0 ? (
        <PermissionSection
          title='Required permissions'
          subtitle='These are necessary for the app to function.'
          groups={requiredGroups}
          editable={editable}
          onToggleEntry={handleToggleEntry}
        />
      ) : null}

      {optionalGroups.length > 0 ? (
        <PermissionSection
          title='Optional permissions'
          subtitle='You can grant these now or keep them disabled.'
          groups={optionalGroups}
          editable={editable}
          collapsed={!showOptional}
          onToggleCollapsed={() => setShowOptional((prev) => !prev)}
          onToggleEntry={handleToggleEntry}
        />
      ) : null}
    </div>
  );
}
