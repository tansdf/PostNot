import { browser } from "$app/environment";
import {
  createCollection,
  createCollectionFolder,
  deleteCollection,
  deleteCollectionItem,
  listCollectionItems,
  listCollections,
  moveCollectionItem,
  saveRequestToCollection,
  updateCollection as updateCollectionCommand,
  updateSavedRequest as updateSavedRequestCommand
} from "$lib/api/commands";
import type {
  CollectionItemSummary,
  CollectionSummary,
  CreateCollectionInput,
  MoveCollectionItemInput,
  RequestDraft
} from "$lib/api/types";
import { notifications } from "$lib/stores/notifications.svelte";

type FolderTarget = {
  id: string | null;
  name: string;
  depth: number;
};

class CollectionsStore {
  initialized = $state(false);
  isCollectionsLoading = $state(false);
  isCollectionItemsLoading = $state(false);
  isCreatingCollection = $state(false);
  isCreatingFolder = $state(false);
  isSavingRequest = $state(false);
  isMovingCollectionItem = $state(false);
  selectedCollectionId = $state("");
  collections = $state.raw<CollectionSummary[]>([]);
  collectionItemsByCollection = $state.raw<Record<string, CollectionItemSummary[]>>({});
  pendingDeleteCollectionId = $state("");
  pendingDeleteCollectionItemId = $state("");
  errorText = $state("");

  get selectedCollection(): CollectionSummary | null {
    return this.collections.find((c) => c.id === this.selectedCollectionId) ?? null;
  }

  get selectedCollectionItems(): CollectionItemSummary[] {
    return this.selectedCollectionId
      ? this.collectionItemsByCollection[this.selectedCollectionId] ?? []
      : [];
  }

  resetError() {
    this.errorText = "";
  }

  folderTargets(collectionId: string): FolderTarget[] {
    if (!collectionId) {
      return [{ id: null, name: "Collection root", depth: 0 }];
    }

    return [
      { id: null, name: "Collection root", depth: 0 },
      ...flattenFolderTargets(this.collectionItemsByCollection[collectionId] ?? [], 0)
    ];
  }

  async ensureLoaded(preferredCollectionId = this.selectedCollectionId) {
    if (!browser) return;
    if (this.initialized && !preferredCollectionId) return;
    await this.loadCollections(preferredCollectionId);
  }

  async loadCollections(preferredCollectionId = this.selectedCollectionId) {
    if (!browser) return;
    this.isCollectionsLoading = true;

    try {
      const fetched = await listCollections();
      const nextId =
        preferredCollectionId && fetched.some((c) => c.id === preferredCollectionId)
          ? preferredCollectionId
          : fetched[0]?.id ?? "";

      this.collections = fetched;
      this.selectedCollectionId = nextId;
      this.initialized = true;
      this.errorText = "";

      if (nextId) {
        await this.loadCollectionItems(nextId);
      } else {
        this.collectionItemsByCollection = {};
      }
    } catch (error) {
      this.initialized = true;
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.isCollectionsLoading = false;
    }
  }

  async loadCollectionItems(collectionId: string) {
    if (!browser || !collectionId) return;
    this.isCollectionItemsLoading = true;

    try {
      const items = await listCollectionItems(collectionId);
      this.collectionItemsByCollection = {
        ...this.collectionItemsByCollection,
        [collectionId]: items
      };
      this.errorText = "";
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.isCollectionItemsLoading = false;
    }
  }

  async selectCollection(collectionId: string) {
    this.selectedCollectionId = collectionId;
    await this.loadCollectionItems(collectionId);
  }

  async createBlankCollection() {
    this.isCreatingCollection = true;

    try {
      const collection = await createCollection({
        name: "Untitled collection",
        description: ""
      });
      await this.loadCollections(collection.id);
      notifications.success(collection.name, "Collection created");
      return collection;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isCreatingCollection = false;
    }
  }

  async createFolder(collectionId: string, name: string, parentId?: string | null) {
    this.isCreatingFolder = true;

    try {
      const folder = await createCollectionFolder(collectionId, {
        name: name.trim(),
        parentId: parentId ?? null
      });
      await Promise.all([this.loadCollections(collectionId), this.loadCollectionItems(collectionId)]);
      notifications.success(folder.name, "Folder created");
      return folder;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isCreatingFolder = false;
    }
  }

  async saveDetails(collectionId: string, input: CreateCollectionInput) {
    try {
      const collection = await updateCollectionCommand(collectionId, input);
      await this.loadCollections(collection.id);
      notifications.success(collection.name, "Collection saved");
      return collection;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    }
  }

  async removeCollection(collectionId: string) {
    this.pendingDeleteCollectionId = collectionId;
    const collectionName = this.collections.find((item) => item.id === collectionId)?.name ?? "Collection";

    try {
      await deleteCollection(collectionId);
      const { [collectionId]: _, ...rest } = this.collectionItemsByCollection;
      this.collectionItemsByCollection = rest;

      const preferredId = this.selectedCollectionId === collectionId ? "" : this.selectedCollectionId;
      await this.loadCollections(preferredId);
      notifications.success(collectionName, "Collection deleted");
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.pendingDeleteCollectionId = "";
    }
  }

  async saveNewRequest(collectionId: string, request: RequestDraft, parentId?: string | null) {
    this.isSavingRequest = true;

    try {
      const savedRequest = await saveRequestToCollection(collectionId, request, parentId);
      await Promise.all([this.loadCollections(collectionId), this.loadCollectionItems(collectionId)]);
      notifications.success(savedRequest.name, "Request saved");
      return savedRequest;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isSavingRequest = false;
    }
  }

  async updateExistingSavedRequest(itemId: string, collectionId: string, request: RequestDraft) {
    this.isSavingRequest = true;

    try {
      const savedRequest = await updateSavedRequestCommand(itemId, request);
      await Promise.all([this.loadCollections(collectionId), this.loadCollectionItems(collectionId)]);
      notifications.success(savedRequest.name, "Request updated");
      return savedRequest;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isSavingRequest = false;
    }
  }

  async removeCollectionItem(collectionId: string, itemId: string, itemName: string) {
    this.pendingDeleteCollectionItemId = itemId;

    try {
      await deleteCollectionItem(itemId);
      await Promise.all([this.loadCollections(collectionId), this.loadCollectionItems(collectionId)]);
      notifications.success(itemName, "Collection item deleted");
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      this.pendingDeleteCollectionItemId = "";
    }
  }

  async moveSavedRequest(
    itemId: string,
    sourceCollectionId: string,
    input: MoveCollectionItemInput
  ) {
    this.isMovingCollectionItem = true;

    try {
      const savedRequest = await moveCollectionItem(itemId, input);
      await this.loadCollections(this.selectedCollectionId || input.targetCollectionId);

      const collectionIds = Array.from(new Set([sourceCollectionId, input.targetCollectionId]));
      await Promise.all(collectionIds.map((collectionId) => this.loadCollectionItems(collectionId)));

      notifications.success(savedRequest.name || "Saved request", "Request moved");
      return savedRequest;
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      return null;
    } finally {
      this.isMovingCollectionItem = false;
    }
  }
}

function flattenFolderTargets(items: CollectionItemSummary[], depth: number): FolderTarget[] {
  return items.flatMap((item) => {
    if (item.kind !== "folder") {
      return [];
    }

    return [
      {
        id: item.id,
        name: item.name,
        depth
      },
      ...flattenFolderTargets(item.children, depth + 1)
    ];
  });
}

export const collections = new CollectionsStore();
