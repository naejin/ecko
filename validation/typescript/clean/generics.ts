// Expected: exit 0
type Nullable<T> = T | null;
type DeepPartial<T> = { [K in keyof T]?: DeepPartial<T[K]> };

interface Repository<T extends { id: string }> {
    findById(id: string): Promise<Nullable<T>>;
    save(entity: T): Promise<T>;
    delete(id: string): Promise<void>;
}

function identity<T>(x: T): T {
    return x;
}

export type { Nullable, DeepPartial, Repository };
export { identity };
