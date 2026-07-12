export type NotificationTone = "info" | "success" | "warning" | "error";

export type NotificationDetails = {
  title: string;
  summary?: string;
  items?: string[];
  warnings?: string[];
  errors?: string[];
  rawText?: string;
};

export type NotificationAction = {
  label: string;
  kind: "details";
};

export type NotificationInput = {
  title?: string;
  message: string;
  tone?: NotificationTone;
  durationMs?: number;
  details?: NotificationDetails;
};

export type NotificationItem = {
  id: string;
  title: string;
  message: string;
  tone: NotificationTone;
  durationMs: number;
  actions: NotificationAction[];
  details: NotificationDetails | null;
};

function createId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `notification-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

class NotificationsStore {
  items = $state.raw<NotificationItem[]>([]);
  maxVisible = 5;
  defaultDurationMs = $state(5_000);
  activeDetails = $state.raw<NotificationDetails | null>(null);

  show(input: NotificationInput) {
    const message = input.message.trim();
    if (!message) {
      return "";
    }

    const notification: NotificationItem = {
      id: createId(),
      title: input.title?.trim() ?? "",
      message,
      tone: input.tone ?? "info",
      durationMs: normalizeDuration(input.durationMs ?? this.defaultDurationMs),
      actions: input.details ? [{ label: "View details", kind: "details" }] : [],
      details: input.details ?? null
    };

    this.items = [notification, ...this.items].slice(0, this.maxVisible);
    return notification.id;
  }

  dismiss(id: string) {
    this.items = this.items.filter((item) => item.id !== id);
  }

  info(message: string, title = "", options: Pick<NotificationInput, "details" | "durationMs"> = {}) {
    return this.show({ tone: "info", title, message, ...options });
  }

  success(message: string, title = "", options: Pick<NotificationInput, "details" | "durationMs"> = {}) {
    return this.show({ tone: "success", title, message, ...options });
  }

  warning(message: string, title = "", options: Pick<NotificationInput, "details" | "durationMs"> = {}) {
    return this.show({ tone: "warning", title, message, ...options });
  }

  error(message: string, title = "", options: Pick<NotificationInput, "details" | "durationMs"> = {}) {
    return this.show({ tone: "error", title, message, ...options });
  }

  openDetails(item: NotificationItem) {
    if (item.details) {
      this.activeDetails = item.details;
    }
  }

  closeDetails() {
    this.activeDetails = null;
  }

  clear() {
    this.items = [];
  }

  setDefaultDuration(durationMs: number) {
    this.defaultDurationMs = normalizeDuration(durationMs);
  }
}

export const notifications = new NotificationsStore();

function normalizeDuration(durationMs: number) {
  return Math.max(1_000, Math.min(60_000, Math.round(durationMs)));
}
