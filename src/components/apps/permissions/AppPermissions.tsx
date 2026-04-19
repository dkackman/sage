import React, { useEffect, useMemo, useState } from 'react';
import {
  AppPermissionGroup,
  type AppPermissionTreeNode,
} from './AppPermissionGroup';
import { SageRequestedPermissions } from '@sage-app/sdk';

type Tone = 'default' | 'added' | 'removed' | 'warning';

interface CapabilityEntry {
  key: string;
  required: boolean;
  granted: boolean;
}

interface Props {
  permissions: SageRequestedPermissions | null | undefined;
  grantedCapabilities?: string[];
  editable?: boolean;
  tone?: Tone;
  onGrantedCapabilitiesChange?: (next: string[]) => void;
}

function buildEntries(
  permissions: SageRequestedPermissions | null | undefined,
  grantedCapabilities: string[],
): CapabilityEntry[] {
  const grantedSet = new Set(grantedCapabilities);

  const requiredEntries = (permissions?.capabilities?.required ?? []).map(
    (key) => ({
      key,
      required: true,
      granted: grantedSet.has(key),
    }),
  );

  const optionalEntries = (permissions?.capabilities?.optional ?? []).map(
    (key) => ({
      key,
      required: false,
      granted: grantedSet.has(key),
    }),
  );

  return [...requiredEntries, ...optionalEntries].sort((a, b) =>
    a.key.localeCompare(b.key),
  );
}

function buildTree(entries: CapabilityEntry[]): AppPermissionTreeNode {
  const root: AppPermissionTreeNode = {
    segment: '',
    fullPath: '',
    children: [],
  };

  for (const entry of entries) {
    const parts = entry.key.split('.');
    let current = root;

    for (let index = 0; index < parts.length; index += 1) {
      const segment = parts[index];
      const fullPath = parts.slice(0, index + 1).join('.');
      const isLeaf = index === parts.length - 1;

      let child = current.children.find((node) => node.segment === segment);

      if (!child) {
        child = {
          segment,
          fullPath,
          children: [],
        };
        current.children.push(child);
      }

      if (isLeaf) {
        child.item = {
          key: entry.key,
          required: entry.required,
          granted: entry.granted,
        };
      }

      current = child;
    }
  }

  currentSort(root);
  return root;
}

function currentSort(node: AppPermissionTreeNode) {
  node.children.sort((a, b) => {
    const aIsGroup = !a.item;
    const bIsGroup = !b.item;

    if (aIsGroup !== bIsGroup) {
      return aIsGroup ? -1 : 1;
    }

    return a.segment.localeCompare(b.segment);
  });

  for (const child of node.children) {
    currentSort(child);
  }
}

export function AppPermissions({
  permissions,
  grantedCapabilities = [],
  editable = false,
  tone = 'default',
  onGrantedCapabilitiesChange,
}: Props) {
  const [localGrantedCapabilities, setLocalGrantedCapabilities] =
    useState<string[]>(grantedCapabilities);

  useEffect(() => {
    setLocalGrantedCapabilities(grantedCapabilities);
  }, [grantedCapabilities]);

  const entries = useMemo(() => {
    return buildEntries(permissions, localGrantedCapabilities);
  }, [permissions, localGrantedCapabilities]);

  const tree = useMemo(() => buildTree(entries), [entries]);

  const hasEntries = entries.length > 0;

  function handleToggle(fullKey: string, nextGranted: boolean) {
    if (!editable) {
      return;
    }

    setLocalGrantedCapabilities((prev) => {
      const prevSet = new Set(prev);

      if (nextGranted) {
        prevSet.add(fullKey);
      } else {
        prevSet.delete(fullKey);
      }

      const requiredKeys = new Set(permissions?.capabilities?.required ?? []);

      for (const key of requiredKeys) {
        prevSet.add(key);
      }

      const next = [...prevSet].sort((a, b) => a.localeCompare(b));

      onGrantedCapabilitiesChange?.(next);
      return next;
    });
  }

  if (!hasEntries) {
    return (
      <div className='text-sm text-muted-foreground'>
        This app does not request any capabilities.
      </div>
    );
  }

  return (
    <AppPermissionGroup
      node={tree}
      editable={editable}
      tone={tone}
      onToggle={handleToggle}
    />
  );
}
