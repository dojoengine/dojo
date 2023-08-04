import { GraphQLClient } from 'graphql-request';
import { GraphQLClientRequestHeaders } from 'graphql-request/build/cjs/types';
import { print } from 'graphql'
import gql from 'graphql-tag';
export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
export type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]?: Maybe<T[SubKey]> };
export type MakeMaybe<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]: Maybe<T[SubKey]> };
export type MakeEmpty<T extends { [key: string]: unknown }, K extends keyof T> = { [_ in K]?: never };
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: { input: string; output: string; }
  String: { input: string; output: string; }
  Boolean: { input: boolean; output: boolean; }
  Int: { input: number; output: number; }
  Float: { input: number; output: number; }
  ContractAddress: { input: any; output: any; }
  DateTime: { input: any; output: any; }
  felt252: { input: any; output: any; }
  u8: { input: any; output: any; }
  u32: { input: any; output: any; }
};

export type ComponentUnion = Moves | Position;

export type Entity = {
  __typename?: 'Entity';
  componentNames: Scalars['String']['output'];
  components?: Maybe<Array<Maybe<ComponentUnion>>>;
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['ID']['output'];
  keys: Scalars['String']['output'];
  updatedAt: Scalars['DateTime']['output'];
};

export type Event = {
  __typename?: 'Event';
  createdAt: Scalars['DateTime']['output'];
  data: Scalars['String']['output'];
  id: Scalars['ID']['output'];
  keys: Scalars['String']['output'];
  systemCall: SystemCall;
  systemCallId: Scalars['Int']['output'];
};

export type Moves = {
  __typename?: 'Moves';
  remaining: Scalars['u8']['output'];
};

export type Position = {
  __typename?: 'Position';
  x: Scalars['u32']['output'];
  y: Scalars['u32']['output'];
};

export type Query = {
  __typename?: 'Query';
  entities?: Maybe<Array<Maybe<Entity>>>;
  entity: Entity;
  event: Event;
  events?: Maybe<Array<Maybe<Event>>>;
  movesComponents?: Maybe<Array<Maybe<Moves>>>;
  positionComponents?: Maybe<Array<Maybe<Position>>>;
  system: System;
  systemCall: SystemCall;
  systemCalls?: Maybe<Array<Maybe<SystemCall>>>;
  systems?: Maybe<Array<Maybe<System>>>;
};


export type QueryEntitiesArgs = {
  componentName?: InputMaybe<Scalars['String']['input']>;
  keys: Array<Scalars['String']['input']>;
  limit?: InputMaybe<Scalars['Int']['input']>;
};


export type QueryEntityArgs = {
  id: Scalars['ID']['input'];
};


export type QueryEventArgs = {
  id: Scalars['ID']['input'];
};


export type QueryEventsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
};


export type QueryMovesComponentsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
  remaining?: InputMaybe<Scalars['u8']['input']>;
};


export type QueryPositionComponentsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
  x?: InputMaybe<Scalars['u32']['input']>;
  y?: InputMaybe<Scalars['u32']['input']>;
};


export type QuerySystemArgs = {
  id: Scalars['ID']['input'];
};


export type QuerySystemCallArgs = {
  id: Scalars['Int']['input'];
};


export type QuerySystemsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
};

export type System = {
  __typename?: 'System';
  address: Scalars['ContractAddress']['output'];
  classHash: Scalars['felt252']['output'];
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['ID']['output'];
  name: Scalars['String']['output'];
  systemCalls: Array<SystemCall>;
  transactionHash: Scalars['felt252']['output'];
};

export type SystemCall = {
  __typename?: 'SystemCall';
  createdAt: Scalars['DateTime']['output'];
  data: Scalars['String']['output'];
  id: Scalars['ID']['output'];
  system: System;
  systemId: Scalars['ID']['output'];
  transactionHash: Scalars['String']['output'];
};

export type GetEntitiesQueryVariables = Exact<{ [key: string]: never; }>;


export type GetEntitiesQuery = { __typename?: 'Query', entities?: Array<{ __typename?: 'Entity', keys: string, components?: Array<{ __typename: 'Moves', remaining: any } | { __typename: 'Position', x: any, y: any } | null> | null } | null> | null };

export type GetEntityMovesQueryVariables = Exact<{
  entityId: Scalars['String']['input'];
}>;


export type GetEntityMovesQuery = { __typename?: 'Query', entities?: Array<{ __typename?: 'Entity', keys: string } | null> | null };

export type GetEntityPositionQueryVariables = Exact<{
  entityId: Scalars['String']['input'];
}>;


export type GetEntityPositionQuery = { __typename?: 'Query', entities?: Array<{ __typename?: 'Entity', keys: string } | null> | null };


export const GetEntitiesDocument = gql`
    query getEntities {
  entities(keys: ["%"]) {
    keys
    components {
      __typename
      ... on Moves {
        remaining
      }
      ... on Position {
        x
        y
      }
    }
  }
}
    `;
export const GetEntityMovesDocument = gql`
    query getEntityMoves($entityId: String!) {
  entities(keys: [$entityId], componentName: "Moves") {
    keys
  }
}
    `;
export const GetEntityPositionDocument = gql`
    query getEntityPosition($entityId: String!) {
  entities(keys: [$entityId], componentName: "Position") {
    keys
  }
}
    `;

export type SdkFunctionWrapper = <T>(action: (requestHeaders?:Record<string, string>) => Promise<T>, operationName: string, operationType?: string) => Promise<T>;


const defaultWrapper: SdkFunctionWrapper = (action, _operationName, _operationType) => action();
const GetEntitiesDocumentString = print(GetEntitiesDocument);
const GetEntityMovesDocumentString = print(GetEntityMovesDocument);
const GetEntityPositionDocumentString = print(GetEntityPositionDocument);
export function getSdk(client: GraphQLClient, withWrapper: SdkFunctionWrapper = defaultWrapper) {
  return {
    getEntities(variables?: GetEntitiesQueryVariables, requestHeaders?: GraphQLClientRequestHeaders): Promise<{ data: GetEntitiesQuery; extensions?: any; headers: Dom.Headers; status: number; }> {
        return withWrapper((wrappedRequestHeaders) => client.rawRequest<GetEntitiesQuery>(GetEntitiesDocumentString, variables, {...requestHeaders, ...wrappedRequestHeaders}), 'getEntities', 'query');
    },
    getEntityMoves(variables: GetEntityMovesQueryVariables, requestHeaders?: GraphQLClientRequestHeaders): Promise<{ data: GetEntityMovesQuery; extensions?: any; headers: Dom.Headers; status: number; }> {
        return withWrapper((wrappedRequestHeaders) => client.rawRequest<GetEntityMovesQuery>(GetEntityMovesDocumentString, variables, {...requestHeaders, ...wrappedRequestHeaders}), 'getEntityMoves', 'query');
    },
    getEntityPosition(variables: GetEntityPositionQueryVariables, requestHeaders?: GraphQLClientRequestHeaders): Promise<{ data: GetEntityPositionQuery; extensions?: any; headers: Dom.Headers; status: number; }> {
        return withWrapper((wrappedRequestHeaders) => client.rawRequest<GetEntityPositionQuery>(GetEntityPositionDocumentString, variables, {...requestHeaders, ...wrappedRequestHeaders}), 'getEntityPosition', 'query');
    }
  };
}
export type Sdk = ReturnType<typeof getSdk>;