import { commands, type SageAppCapabilityDefinitionView } from '@/bindings';

let capabilityRegistryPromise: Promise<
  Record<string, SageAppCapabilityDefinitionView>
> | null = null;

export async function getCapabilityRegistry() {
  if (!capabilityRegistryPromise) {
    capabilityRegistryPromise = commands
      .appsGetCapabilityRegistry()
      .then((entries) =>
        Object.fromEntries(entries.map((entry) => [entry.key, entry])),
      );
  }

  return capabilityRegistryPromise;
}

