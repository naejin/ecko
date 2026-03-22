// Expected: exit 0
import { readFileSync, existsSync } from 'fs';
import { join } from 'path';

interface Config {
    name: string;
    port: number;
}

function loadConfig(dir: string): Config | null {
    const filePath = join(dir, 'config.json');
    if (!existsSync(filePath)) {
        return null;
    }
    try {
        const content = readFileSync(filePath, 'utf-8');
        return JSON.parse(content) as Config;
    } catch (err) {
        console.error('Failed to load config:', err);
        return null;
    }
}

export { loadConfig };
