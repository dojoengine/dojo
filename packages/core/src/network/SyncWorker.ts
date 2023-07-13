import { Components } from "@latticexyz/recs";
import { Providers } from "..";
import { setComponentFromEntitiesGraphqlQuery, setComponentFromEntitiesQuery, setComponentFromEvent } from "../utils";
import { Event } from "starknet";
import { getEntities } from "./graphql";

export class SyncWorker<C extends Components> {
  private provider: Providers.RPCProvider;
  private components: C;
  private event_key: string;
  private url?: string;
  private useIndexer?: boolean;
  private waitForTx?: boolean;


  constructor(provider: Providers.RPCProvider, components: C, event_key: string, url: string, useIndexer: boolean = false, waitForTx: boolean = true) {
    console.log("Creating SyncWorker...");
    this.provider = provider;
    this.components = components;
    this.event_key = event_key;
    this.url = url;
    this.useIndexer = useIndexer;
    this.waitForTx = waitForTx;
    this.init();
  }

  private async init() {
   for (const key of Object.keys(this.components)) {
        const component = this.components[key];
        if (component.metadata && component.metadata.name) {
            if (this.useIndexer && this.url) {
                const entities = await getEntities(this.url, component.metadata.name as string, component.schema);
                entities && setComponentFromEntitiesGraphqlQuery(component, entities);
            } else {
                // call provider.entities for each component to get all entities linked to that component
                const entities = await this.provider.entities(component.metadata.name as string, "0", Object.keys(component.schema).length);
                setComponentFromEntitiesQuery(component, entities);
            }
            }
        }
    console.log('SyncWorker initialized');
    }


    public async sync(txHash: string) {
        if (this.waitForTx) {
            await this.provider.provider.waitForTransaction(txHash);
        }
        this.provider.provider.getTransactionReceipt(txHash).then((receipt) => {
            receipt.events.filter((event) => {
                return event.keys.length === 1 &&
                event.keys[0] === this.event_key;
        }
        ).map((event: Event) => {
            setComponentFromEvent(this.components, event.data);
            });

        })
    }
}