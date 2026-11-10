export interface ServerInfo {
  url: string
  host: string
  port: number
  token: string
  inbox_dir: string
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

export type Tab = 'connect' | 'files' | 'clips' | 'security' | 'settings'
export type ConfirmKind = 'file' | 'clips-clear' | 'files-clear' | 'files-batch'
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
