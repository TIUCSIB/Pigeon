export interface ServerInfo {
  url: string
  host: string
  port: number
  token: string
  inbox_dir: string
  token_enabled?: boolean
  auth_enabled?: boolean
  sharing?: boolean
}

/** 连接过的设备(服务端按来源 IP 汇总) */
export interface DeviceEntry {
  ip: string
  ua: string
  username: string | null
  first_seen: number
  last_seen: number
  /** 是否有活跃的 SSE 连接(在线) */
  sse: boolean
  uploads: number
  downloads: number
}

/** 一次传输记录 */
export interface TransferRecord {
  id: number
  time: number
  /** in = 手机→电脑 | out = 电脑→手机 */
  dir: 'in' | 'out'
  name: string
  size: number
  ip: string
}

export interface FileEntry {
  name: string
  size: number
  modified: number
}

export interface Clip {
  id: number
  content: string
  created: number
  source: string
}

export interface Settings {
  port: number
  inbox_dir: string | null
  sharing?: boolean
  dark_mode?: boolean
  token_enabled?: boolean
  auth_enabled?: boolean
  max_concurrent_uploads?: number
  speed_limit_mbps?: number
}

export interface AccountView {
  username: string
  allow_download: boolean
  allow_upload_file: boolean
  allow_upload_text: boolean
  allow_delete: boolean
}

export interface FrozenEntry {
  name: string
  size: number
}

export type Tab = 'connect' | 'files' | 'clips' | 'devices' | 'security' | 'settings'
export type ConfirmKind = 'file' | 'clips-clear' | 'files-clear' | 'files-batch' | 'history-clear'
export interface AccountInput {
  username: string
  password: string
  allowDownload: boolean
  allowUploadFile: boolean
  allowUploadText: boolean
  allowDelete: boolean
}

export interface Toast {
  id: number
  text: string
}
