import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

export async function openAppWindow(appId: string, appName: string) {
  const label = `app-${appId}`;

  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }

  const win = new WebviewWindow(label, {
    url: `sage-app://${appId}/index.html`,
    title: appName,
    width: 1000,
    height: 700,
    resizable: true,
  });

  await win.once('tauri://created', async () => {
    await win.setFocus();
  });
}

