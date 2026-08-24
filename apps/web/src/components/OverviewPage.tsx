import { IconDatabase } from "./icons"
import { Card, formatWhen, PageHeader, StatTile, useConfig } from "./shared"

export default function OverviewPage() {
  const config = useConfig()
  if (config.isPending) return <OverviewSkeleton />
  if (config.isError) {
    return (
      <div className="border-danger/30 bg-danger-dim text-danger rounded-xl border px-4 py-3 text-sm">
        {config.error.message}
      </div>
    )
  }
  const data = config.data
  const enabled = data.sources.filter((source) => source.enabled).length
  const byPolicy = countPolicies(data.outlets)
  const lastCheck = data.runtime_config["last_check"]
  const maxItems = data.runtime_config["max_delivery_items"] ?? "default"

  return (
    <>
      <PageHeader
        title="Overview"
        subtitle="Local brief pipeline at a glance"
      />
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatTile
          label="Sources"
          value={data.sources.length}
          hint={`${enabled} enabled · ${data.sources.length - enabled} disabled`}
        />
        <StatTile
          label="Outlet policies"
          value={data.outlets.length}
          hint={`${byPolicy["allow"]} allow · ${byPolicy["block"]} block · ${byPolicy["watch"]} watch`}
        />
        <StatTile
          label="Max items"
          value={maxItems}
          hint="per delivered brief"
        />
        <StatTile
          label="Last check"
          value={lastCheck ? formatWhen(lastCheck) : "Not recorded"}
          hint="scheduler heartbeat"
        />
      </div>
      <Card title="Storage" className="mt-4">
        <dl className="divide-edge divide-y">
          <StorageRow label="Database" value={data.paths.database_path} />
          <StorageRow label="Data directory" value={data.paths.data_dir} />
        </dl>
      </Card>
    </>
  )
}

function OverviewSkeleton() {
  return (
    <>
      <div className="mb-6">
        <div className="bg-raised h-6 w-32 animate-pulse rounded" />
      </div>
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        {[1, 2, 3, 4].map((key) => (
          <div key={key} className="bg-surface h-24 animate-pulse rounded-xl" />
        ))}
      </div>
      <div className="bg-surface mt-4 h-32 animate-pulse rounded-xl" />
    </>
  )
}

type PolicyCounts = { allow: number; block: number; watch: number }

function countPolicies(outlets: { policy: string }[]): PolicyCounts {
  const counts: PolicyCounts = { allow: 0, block: 0, watch: 0 }
  for (const outlet of outlets) {
    if (outlet.policy === "allow") counts.allow += 1
    if (outlet.policy === "block") counts.block += 1
    if (outlet.policy === "watch") counts.watch += 1
  }
  return counts
}

function StorageRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col items-start gap-1 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
      <dt className="text-muted flex items-center gap-2 text-sm">
        <IconDatabase className="h-3.5 w-3.5" />
        {label}
      </dt>
      <dd className="text-ink max-w-full font-mono text-xs break-words sm:text-right">
        {value}
      </dd>
    </div>
  )
}
