/** The dock's half of the bridge. Separate from the settings window's API. */

export interface AppWindow {
  hwnd: number;
  title: string;
  exe: string;
  minimized: boolean;
  maximized: boolean;
}

export interface DockItem {
  exe: string;
  name: string;
  pinned: boolean;
  windows: AppWindow[];
}

export interface DockLayout {
  work_x: number;
  work_y: number;
  work_width: number;
  work_height: number;
  icon_size: number;
}

export interface DockConfig {
  enabled: boolean;
  pinned: string[];
  icon_size: number;
}

export interface IconData {
  width: number;
  height: number;
  rgba_base64: string;
}

const invoke = async <T,>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  const { invoke: call } = await import("@tauri-apps/api/core");
  return call<T>(cmd, args);
};

export const dockApi = {
  layout: () => invoke<DockLayout>("dock_layout"),
  items: () => invoke<DockItem[]>("dock_items"),
  icon: (exe: string, size: number) => invoke<IconData | null>("dock_icon", { exe, size }),
  activate: (hwnd: number) => invoke<boolean>("dock_activate", { hwnd }),
  launch: (target: string) => invoke<boolean>("dock_launch", { target }),
  place: (width: number, height: number) => invoke<void>("dock_place", { width, height }),
  config: () => invoke<DockConfig>("dock_config"),
  minimize: (hwnd: number) => invoke<boolean>("dock_minimize", { hwnd }),
  toggleMaximize: (hwnd: number) => invoke<boolean>("dock_toggle_maximize", { hwnd }),
  close: (hwnd: number) => invoke<boolean>("dock_close", { hwnd }),
  pin: (exe: string) => invoke<DockConfig>("dock_pin", { exe }),
  unpin: (exe: string) => invoke<DockConfig>("dock_unpin", { exe }),
};
