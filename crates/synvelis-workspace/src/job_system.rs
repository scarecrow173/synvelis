//! Job-system composition (synvelis#518) — the single place hosts
//! assemble the render scheduler and its gateway-facing adapter.
//!
//! [`JobSystem::headless`] is the honest minimum for hosts without
//! render backends, settings documents, or declared policies: the
//! scheduler runs with fail-closed ports, so `synvelis.render.*`
//! commands produce typed rejections/failures — never fabricated
//! success. Hosts with real ports compose [`JobSystem::with_ports`]
//! instead; the scheduler contract (bounded queue, single writer,
//! demand-granularity interleaving) is identical either way.
//!
//! The returned [`WorkspaceRenderDispatch`] implements the gateway's
//! host-injected [`synvelis_gateway::variant::RenderDispatch`] port —
//! synvelis-gateway stays independent of synvelis-render; this crate
//! is the seam that names both.

use std::sync::Arc;

use synvelis_gateway::variant::RenderDispatch;
use synvelis_render::policy::model::LocalGrants;
use synvelis_render::render_job::{JobScheduler, SchedulerPorts};

use crate::render_dispatch::WorkspaceRenderDispatch;

/// Owns the render [`JobScheduler`] and vends its gateway adapter.
/// Clone shares the same scheduler worker.
#[derive(Clone)]
pub struct JobSystem {
    scheduler: JobScheduler,
}

impl JobSystem {
    /// Headless composition — in-memory artifact store, cpu-only
    /// grants, fail-closed ports. See [`JobScheduler::headless`].
    #[must_use]
    pub fn headless() -> Self {
        Self {
            scheduler: JobScheduler::headless(),
        }
    }

    /// Composes a scheduler over caller-supplied ports and grants —
    /// the production path for hosts that wire real executors,
    /// settings probes, policy resolvers, and demand resolvers.
    #[must_use]
    pub fn with_ports(ports: SchedulerPorts, grants: LocalGrants, queue_capacity: usize) -> Self {
        Self {
            scheduler: JobScheduler::spawn(ports, grants, queue_capacity),
        }
    }

    /// Wraps an already-spawned scheduler (tests, embedded hosts).
    #[must_use]
    pub fn with_scheduler(scheduler: JobScheduler) -> Self {
        Self { scheduler }
    }

    /// The underlying scheduler handle — status/control surfaces
    /// that need typed job views or receipt streams use this
    /// directly.
    #[must_use]
    pub fn scheduler(&self) -> &JobScheduler {
        &self.scheduler
    }

    /// The gateway-facing adapter as `Arc<dyn RenderDispatch +
    /// Send + Sync>` — the value hosts assign to `GateDeps.render`.
    #[must_use]
    pub fn dispatch(&self) -> Arc<dyn RenderDispatch + Send + Sync> {
        WorkspaceRenderDispatch::shared(self.scheduler.clone())
    }
}
