import { useConfig } from "./shared"

export default function OverviewPage() {
  const config = useConfig()
  if (config.isPending) return <p className="text-(--muted)">Loading…</p>
  if (config.isError) {
    return <p className="text-sm text-red-400">{config.error.message}</p>
  }
  const data = config.data
  const lastCheck = data.runtime_config["last_check"] ?? "unknown"
  const maxItems = data.runtime_config["max_delivery_items"] ?? "default"
  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        <Metric label="Sources" value={String(data.sources.length)} />
        <Metric label="Outlet policies" value={String(data.outlets.length)} />
        <Metric label="Max items" value={maxItems} />
        <Metric label="Last check" value={lastCheck} />
      </div>
      <section className="rounded-lg border border-(--border) bg-(--surface) p-4 text-sm">
        <h2 className="mb-2 font-semibold text-(--muted) uppercase">Storage</h2>
        <p>Database: {data.paths.database_path}</p>
        <p>Data directory: {data.paths.data_dir}</p>
      </section>
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-(--border) bg-(--surface) p-4">
      <p className="text-xs text-(--muted) uppercase">{label}</p>
      <p className="text-xl font-semibold break-all">{value}</p>
    </div>
  )
}
