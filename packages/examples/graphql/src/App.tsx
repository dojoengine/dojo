import { GraphQLClient } from 'graphql-request';
import './App.css';
import { getSdk } from './generated/graphql';
import { useEffect } from 'react';

const client = new GraphQLClient('http://localhost:8080');
const sdk = getSdk(client);

function App() {

  useEffect(() => {
    const fetchData = async () => {
      const { data } = await sdk.getEntities();
      console.log(data);
    }

    fetchData();
  }, []);


  return (
    <div>
      <h4>Dojo Graphql Example</h4>
    </div>
  );
}

export default App;
